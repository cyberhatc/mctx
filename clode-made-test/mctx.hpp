#pragma once
extern "C" {
#include "mctx.h"
}
#include <string>
#include <vector>
#include <stdexcept>

namespace mctx {

struct Section {
    std::string name;
    std::string tier;
    int version;
    long offset;
};

class Store {
public:
    explicit Store(std::string path) : path_(std::move(path)) { reload(); }

    void reload() {
        if (mctx_load_index(path_.c_str(), &idx_) < 0)
            throw std::runtime_error("mctx: could not open " + path_);
    }

    std::vector<Section> index() const {
        std::vector<Section> out;
        for (int i = 0; i < idx_.count; i++) {
            const auto &e = idx_.entries[i];
            out.push_back({e.name, e.tier, e.version, e.offset});
        }
        return out;
    }

    /* Direct seek-and-read via the byte offset — never loads the whole file. */
    std::string read(const std::string &name, size_t max_bytes = 65536) const {
        std::string buf(max_bytes, '\0');
        long n = mctx_read_section(path_.c_str(), &idx_, name.c_str(), &buf[0], max_bytes);
        if (n < 0) throw std::runtime_error("mctx: section not found: " + name);
        buf.resize((size_t)n);
        return buf;
    }

    /* Update if it exists, create it if it doesn't. Bumps version and
       rebuilds the index automatically. */
    void write(const std::string &name, const std::string &tier, const std::string &body) {
        if (mctx_write_section(path_.c_str(), name.c_str(), tier.c_str(), body.c_str()) != 0)
            throw std::runtime_error("mctx: write failed for " + name);
        reload();
    }

    /* Convenience for the checkpoint pattern: "I'm about to run out of
       tokens, save my state" — always !volatile, always appended fresh. */
    void checkpoint(const std::string &body) {
        write("checkpoint", "!volatile", body);
    }

private:
    std::string path_;
    MctxIndex idx_{};
};

} // namespace mctx
