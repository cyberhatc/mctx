#ifndef MCTX_H
#define MCTX_H

#include <stdio.h>
#include <stdlib.h>

#define MCTX_MAX_SECTIONS 128
#define MCTX_NAME_LEN     64
#define MCTX_TIER_LEN     16

/* One row in the %%INDEX block: name, durability tier, version, byte offset
   of the section's "%%@name" marker line in the file. The offset is what
   makes lookups O(1)-seek instead of O(file size) scan. */
typedef struct {
    char name[MCTX_NAME_LEN];
    char tier[MCTX_TIER_LEN];   /* "!fixed" | "!durable" | "!volatile" */
    int  version;
    long offset;
} MctxIndexEntry;

typedef struct {
    MctxIndexEntry entries[MCTX_MAX_SECTIONS];
    int count;
} MctxIndex;

/* Read only the %%INDEX ... %%END-INDEX block at the top of the file.
   Cheap even on a huge memory file, since it never touches section bodies. */
int mctx_load_index(const char *path, MctxIndex *idx);

/* Seek straight to a section's offset (from the index) and read its body
   (between "%%@name ..." and "%%END") into out_buf. Returns bytes read,
   or -1 if the section isn't in the index. Does not read the rest of the file. */
long mctx_read_section(const char *path, const MctxIndex *idx,
                        const char *name, char *out_buf, size_t out_buf_size);

/* Replace a section's body with new_body, bump its version, and rebuild the
   index to reflect any offset shifts. Rewrites the file (sections after the
   edited one may move, so a full rewrite is the only way to stay correct). */
int mctx_write_section(const char *path, const char *name,
                        const char *tier, const char *new_body);

/* Append a brand-new section (e.g. an agent's task checkpoint) and rebuild
   the index. If the section already exists, use mctx_write_section instead. */
int mctx_append_section(const char *path, const char *name,
                         const char *tier, const char *body);

/* Rescan the whole file for "%%@name" markers and regenerate the %%INDEX
   block with fresh byte offsets. Call this any time section bodies were
   edited by hand / by another tool instead of through this API. */
int mctx_rebuild_index(const char *path);

#endif
