#ifndef GOLDEN_INFLATE_H
#define GOLDEN_INFLATE_H

#include <stddef.h>
#include <stdint.h>

/* Inflates a zlib stream (PNG IDAT). Returns 0 and writes *out_len on success. */
int golden_inflate(const uint8_t *src, size_t src_len, uint8_t *dst, size_t dst_len, size_t *out_len);

#endif
