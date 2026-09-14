#include "golden_inflate.h"

#include <zlib.h>

int golden_inflate(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_len, size_t *out_len) {
    z_stream strm = {0};
    if (inflateInit(&strm) != Z_OK) {
        return -1;
    }
    strm.next_in = (Bytef *)src;
    strm.avail_in = (uInt)src_len;
    strm.next_out = dst;
    strm.avail_out = (uInt)dst_len;
    int ret = inflate(&strm, Z_FINISH);
    *out_len = strm.total_out;
    inflateEnd(&strm);
    return ret == Z_STREAM_END ? 0 : ret;
}
