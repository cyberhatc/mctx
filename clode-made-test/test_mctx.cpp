#include "mctx.hpp"
#include <iostream>
#include <cassert>

int main() {
    const std::string path = "/home/claude/mctx/sample2.mctx";
    { FILE *f = fopen(path.c_str(), "w"); fputs("#mctx v1.1 | updated:2026-08-08\n", f); fclose(f); }

    mctx::Store store(path);
    mctx_append_section(path.c_str(), "identity", "!fixed",
        "user{alias,role}:\n  \"devil2\",\"builder\"\n");
    store.reload();

    store.checkpoint("task: implement mctx C++ wrapper\nnext: write rust port\nfiles_touched: mctx.hpp\n");

    for (auto &s : store.index())
        std::cout << s.name << " " << s.tier << " v" << s.version << " @" << s.offset << "\n";

    auto body = store.read("checkpoint");
    std::cout << "\n--- checkpoint ---\n" << body << "\n";
    assert(body.find("write rust port") != std::string::npos);

    std::cout << "OK — C++ wrapper works.\n";
    return 0;
}
