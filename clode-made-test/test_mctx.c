#include "mctx.h"
#include <string.h>
#include <assert.h>

int main(void) {
    const char *path = "/home/claude/mctx/sample.mctx";
    FILE *f = fopen(path, "w");
    fprintf(f, "#mctx v1.1 | updated:2026-08-08\n");
    fclose(f);

    mctx_append_section(path, "identity", "!fixed",
        "user{alias,role,base}:\n  \"devil2\",\"student/builder\",\"India\"\n");
    mctx_append_section(path, "projects", "!durable",
        "projects[1]{id,title,status}:\n  p1,\"SmartTodo\",\"in progress\"\n");
    mctx_append_section(path, "log", "!volatile",
        "memories[1]{date,fact}:\n  \"2026-08-08\",\"built the mctx C lib\"\n");

    MctxIndex idx;
    int n = mctx_load_index(path, &idx);
    printf("indexed sections: %d\n", n);
    for (int i = 0; i < idx.count; i++) {
        printf("  %-10s %-10s v%d offset=%ld\n",
               idx.entries[i].name, idx.entries[i].tier,
               idx.entries[i].version, idx.entries[i].offset);
    }

    char buf[1024];
    long len = mctx_read_section(path, &idx, "projects", buf, sizeof(buf));
    printf("\n--- direct seek read of 'projects' (%ld bytes) ---\n%s\n", len, buf);
    assert(len > 0);
    assert(strstr(buf, "SmartTodo") != NULL);

    mctx_write_section(path, "projects", "!durable",
        "projects[1]{id,title,status}:\n  p1,\"SmartTodo\",\"shipped\"\n");

    mctx_load_index(path, &idx);
    len = mctx_read_section(path, &idx, "projects", buf, sizeof(buf));
    printf("--- after update, v should be 2 ---\n%s\n", buf);
    assert(strstr(buf, "shipped") != NULL);
    for (int i = 0; i < idx.count; i++) {
        if (strcmp(idx.entries[i].name, "projects") == 0) {
            printf("projects version now: %d\n", idx.entries[i].version);
            assert(idx.entries[i].version == 2);
        }
    }

    printf("\nOK — all assertions passed.\n");
    return 0;
}
