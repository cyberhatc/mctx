#include "mctx.h"
#include <string.h>

/* ---------- internal helpers ---------- */

static char *mctx_read_whole_file(const char *path, long *out_size) {
    FILE *f = fopen(path, "rb");
    if (!f) return NULL;
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    rewind(f);
    char *buf = (char *)malloc((size_t)size + 1);
    if (!buf) { fclose(f); return NULL; }
    size_t n = fread(buf, 1, (size_t)size, f);
    buf[n] = '\0';
    fclose(f);
    if (out_size) *out_size = (long)n;
    return buf;
}

static int mctx_write_whole_file(const char *path, const char *content) {
    FILE *f = fopen(path, "wb");
    if (!f) return -1;
    fputs(content, f);
    fclose(f);
    return 0;
}

/* Parse one "%%@name !tier v:N" marker line. Returns 1 on success. */
static int mctx_parse_marker(const char *line, char *name, char *tier, int *version) {
    if (strncmp(line, "%%@", 3) != 0) return 0;
    *version = 1;
    tier[0] = '\0';
    int n = sscanf(line + 3, "%63[^ \t\r\n] %15[^ \t\r\n] v:%d", name, tier, version);
    return n >= 1;
}

/* ---------- public API ---------- */

int mctx_load_index(const char *path, MctxIndex *idx) {
    idx->count = 0;
    FILE *f = fopen(path, "r");
    if (!f) return -1;

    char line[512];
    int in_index = 0;
    while (fgets(line, sizeof(line), f)) {
        if (strncmp(line, "%%INDEX", 7) == 0) { in_index = 1; continue; }
        if (strncmp(line, "%%END-INDEX", 11) == 0) break;
        if (!in_index) continue;

        MctxIndexEntry *e = &idx->entries[idx->count];
        /* "name:tier:vN:offset" */
        char verbuf[16];
        if (sscanf(line, "%63[^:]:%15[^:]:v%d:%ld",
                   e->name, e->tier, &e->version, &e->offset) == 4 ||
            sscanf(line, "%63[^:]:%15[^:]:%15[^:]:%ld",
                   e->name, e->tier, verbuf, &e->offset) == 4) {
            if (idx->count < MCTX_MAX_SECTIONS) idx->count++;
        }
    }
    fclose(f);
    return idx->count;
}

long mctx_read_section(const char *path, const MctxIndex *idx,
                        const char *name, char *out_buf, size_t out_buf_size) {
    long offset = -1;
    for (int i = 0; i < idx->count; i++) {
        if (strcmp(idx->entries[i].name, name) == 0) { offset = idx->entries[i].offset; break; }
    }
    if (offset < 0) return -1;

    FILE *f = fopen(path, "r");
    if (!f) return -1;
    fseek(f, offset, SEEK_SET);

    char line[512];
    size_t written = 0;
    /* skip the marker line itself */
    if (!fgets(line, sizeof(line), f)) { fclose(f); return -1; }

    while (fgets(line, sizeof(line), f)) {
        if (strncmp(line, "%%END", 5) == 0) break;
        size_t len = strlen(line);
        if (written + len >= out_buf_size) len = out_buf_size - written - 1;
        memcpy(out_buf + written, line, len);
        written += len;
        if (written >= out_buf_size - 1) break;
    }
    out_buf[written] = '\0';
    fclose(f);
    return (long)written;
}

int mctx_rebuild_index(const char *path) {
    long size;
    char *content = mctx_read_whole_file(path, &size);
    if (!content) return -1;

    /* Strip existing index block, if present. */
    char *idx_start = strstr(content, "%%INDEX");
    char *idx_end = strstr(content, "%%END-INDEX");
    char *body_start = content; /* where section markers begin, post-index */
    char header[256] = "";

    if (idx_start && idx_end) {
        size_t hdr_len = (size_t)(idx_start - content);
        if (hdr_len >= sizeof(header)) hdr_len = sizeof(header) - 1;
        memcpy(header, content, hdr_len);
        header[hdr_len] = '\0';
        body_start = idx_end + strlen("%%END-INDEX");
        while (*body_start == '\r' || *body_start == '\n') body_start++;
    } else {
        /* no index yet: first line is the header */
        char *nl = strchr(content, '\n');
        size_t hdr_len = nl ? (size_t)(nl - content + 1) : 0;
        if (hdr_len >= sizeof(header)) hdr_len = sizeof(header) - 1;
        memcpy(header, content, hdr_len);
        header[hdr_len] = '\0';
        body_start = content + hdr_len;
    }

    /* Scan body for markers, recording offsets relative to the FINAL file
       (header + new index block + body), computed after we know index size. */
    char names[MCTX_MAX_SECTIONS][MCTX_NAME_LEN];
    char tiers[MCTX_MAX_SECTIONS][MCTX_TIER_LEN];
    int versions[MCTX_MAX_SECTIONS];
    long rel_offsets[MCTX_MAX_SECTIONS];
    int count = 0;

    char *p = body_start;
    while ((p = strstr(p, "%%@")) != NULL) {
        char name[MCTX_NAME_LEN] = "", tier[MCTX_TIER_LEN] = "";
        int version = 1;
        if (mctx_parse_marker(p, name, tier, &version) && count < MCTX_MAX_SECTIONS) {
            strncpy(names[count], name, MCTX_NAME_LEN - 1);
            strncpy(tiers[count], tier[0] ? tier : "!durable", MCTX_TIER_LEN - 1);
            versions[count] = version;
            rel_offsets[count] = (long)(p - body_start); /* offset within body */
            count++;
        }
        p += 3;
    }

    /* Offsets are zero-padded to a FIXED width (%010ld) specifically so the
       index block's byte length doesn't depend on the offset values it
       contains -- otherwise this is circular (block length determines
       offsets, offsets' digit-width changes block length). Fixed width
       breaks the cycle: a dummy pass and the real pass are always the same
       length, so 'base' computed from the dummy pass is exact. */
    char index_block[4096];
    int off = 0;
    off += snprintf(index_block + off, sizeof(index_block) - off, "%%%%INDEX\n");
    for (int i = 0; i < count; i++) {
        off += snprintf(index_block + off, sizeof(index_block) - off,
                         "%s:%s:v%d:%010d\n", names[i], tiers[i], versions[i], 0);
    }
    off += snprintf(index_block + off, sizeof(index_block) - off, "%%%%END-INDEX\n");

    long index_block_len = strlen(index_block);
    long base = (long)strlen(header) + index_block_len;

    char final_index[4096];
    off = 0;
    off += snprintf(final_index + off, sizeof(final_index) - off, "%%%%INDEX\n");
    for (int i = 0; i < count; i++) {
        long abs_offset = base + rel_offsets[i];
        off += snprintf(final_index + off, sizeof(final_index) - off,
                         "%s:%s:v%d:%010ld\n", names[i], tiers[i], versions[i], abs_offset);
    }
    off += snprintf(final_index + off, sizeof(final_index) - off, "%%%%END-INDEX\n");

    size_t total_len = strlen(header) + strlen(final_index) + strlen(body_start) + 1;
    char *out = (char *)malloc(total_len);
    snprintf(out, total_len, "%s%s%s", header, final_index, body_start);

    int rc = mctx_write_whole_file(path, out);
    free(out);
    free(content);
    return rc;
}

int mctx_write_section(const char *path, const char *name,
                        const char *tier, const char *new_body) {
    long size;
    char *content = mctx_read_whole_file(path, &size);
    if (!content) return -1;

    char marker_prefix[MCTX_NAME_LEN + 4];
    snprintf(marker_prefix, sizeof(marker_prefix), "%%%%@%s", name);
    char *sec_start = strstr(content, marker_prefix);
    if (!sec_start) { free(content); return mctx_append_section(path, name, tier, new_body); }

    char *sec_end = strstr(sec_start, "%%END");
    if (!sec_end) { free(content); return -1; }
    sec_end += strlen("%%END");
    while (*sec_end == '\r' || *sec_end == '\n') sec_end++;

    char old_name[MCTX_NAME_LEN] = "", old_tier[MCTX_TIER_LEN] = "";
    int version = 1;
    mctx_parse_marker(sec_start, old_name, old_tier, &version);
    version++;
    const char *use_tier = (tier && tier[0]) ? tier : (old_tier[0] ? old_tier : "!durable");

    char new_marker[512];
    snprintf(new_marker, sizeof(new_marker), "%%%%@%s %s v:%d\n%s%%%%END\n",
             name, use_tier, version, new_body);

    size_t pre_len = (size_t)(sec_start - content);
    size_t total_len = pre_len + strlen(new_marker) + strlen(sec_end) + 1;
    char *out = (char *)malloc(total_len);
    memcpy(out, content, pre_len);
    out[pre_len] = '\0';
    strcat(out, new_marker);
    strcat(out, sec_end);

    int rc = mctx_write_whole_file(path, out);
    free(out);
    free(content);
    if (rc == 0) rc = mctx_rebuild_index(path);
    return rc;
}

int mctx_append_section(const char *path, const char *name,
                         const char *tier, const char *body) {
    FILE *f = fopen(path, "a");
    if (!f) return -1;
    fprintf(f, "\n%%%%@%s %s v:1\n%s%%%%END\n", name, tier ? tier : "!durable", body);
    fclose(f);
    return mctx_rebuild_index(path);
}
