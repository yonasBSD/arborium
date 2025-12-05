 * allocated buffer was larger than the maximum size supported by a type, `alloc` 
 * would be truncated, creating a mismatch between `alloc` and the actual buffer size.
 */
char sdsReqType(size_t string_size) {
    if (string_size < 1 << 5) return SDS_TYPE_5;
    if (string_size <= (1 << 8) - sizeof(struct sdshdr8) - 1) return SDS_TYPE_8;
    if (string_size <= (1 << 16) - sizeof(struct sdshdr16) - 1) return SDS_TYPE_16;
#if (LONG_MAX == LLONG_MAX)
    if (string_size <= (1ll << 32) - sizeof(struct sdshdr32) - 1) return SDS_TYPE_32;
    return SDS_TYPE_64;
#else
    return SDS_TYPE_32;
#endif
}

static inline size_t sdsTypeMaxSize(char type) {
    if (type == SDS_TYPE_5)
        return (1<<5) - 1;
    if (type == SDS_TYPE_8)
        return (1<<8) - 1;
    if (type == SDS_TYPE_16)
        return (1<<16) - 1;
#if (LONG_MAX == LLONG_MAX)
    if (type == SDS_TYPE_32)
        return (1ll<<32) - 1;
#endif
    return -1; /* this is equivalent to the max SDS_TYPE_64 or SDS_TYPE_32 */
}

/* 
 * Adjusts the SDS type if the allocated buffer size exceeds the maximum size 
 * addressable by the current type.
 *
 * The SDS type is initially determined based on the logical length of the string. 
 * However, allocators like jemalloc may return a buffer larger than requested, 
 * potentially exceeding the maximum size the selected SDS type can handle. This 
 * can cause a mismatch between the `alloc` field and the actual buffer size, 
 * leading to wasted memory and possible inconsistencies.
 *
 * This function ensures that the SDS type is selected based on the actual buffer 
 * size rather than just the logical length. If the buffer size supports a larger 
 * SDS type, it updates `type` and `hdrlen` accordingly.
 *
 * Returns 1 if the type was adjusted, 0 otherwise.
 */
static inline int adjustTypeIfNeeded(char *type, int *hdrlen, size_t bufsize) {
    size_t usable = bufsize - *hdrlen - 1;
    if (*type != SDS_TYPE_5 && usable > sdsTypeMaxSize(*type)) {
        *type = sdsReqType(usable);
        *hdrlen = sdsHdrSize(*type);
        return 1;
    }
    return 0;
}

/* Create a new sds string with the content specified by the 'init' pointer
 * and 'initlen'.
 * If NULL is used for 'init' the string is initialized with zero bytes.
 * If SDS_NOINIT is used, the buffer is left uninitialized;
 *
 * The string is always null-terminated (all the sds strings are, always) so
 * even if you create an sds string with:
 *
 * mystring = sdsnewlen("abc",3);
 *
 * You can print the string with printf() as there is an implicit \0 at the
 * end of the string. However the string is binary safe and can contain
 * \0 characters in the middle, as the length is stored in the sds header. */
sds _sdsnewlen(const void *init, size_t initlen, int trymalloc) {
    void *sh;

    char type = sdsReqType(initlen);
    /* Empty strings are usually created in order to append. Use type 8
     * since type 5 is not good at this. */
    if (type == SDS_TYPE_5 && initlen == 0) type = SDS_TYPE_8;
    int hdrlen = sdsHdrSize(type);
    size_t bufsize;

    assert(initlen + hdrlen + 1 > initlen); /* Catch size_t overflow */
    sh = trymalloc?
        s_trymalloc_usable(hdrlen+initlen+1, &bufsize) :
        s_malloc_usable(hdrlen+initlen+1, &bufsize);
    if (sh == NULL) return NULL;

    adjustTypeIfNeeded(&type, &hdrlen, bufsize);
    return sdsnewplacement(sh, bufsize, type, init, initlen);
}

/* Initializes an SDS within pre-allocated buffer. Like, placement new in C++. 
 * 
 * Parameters:
 * - `buf`    : A pre-allocated buffer for the SDS.
 * - `bufsize`: Total size of the buffer (>= `sdsReqSize(initlen, type)`). Can use 
 *              a larger `bufsize` than required, but usable size won't be greater 
 *              than `sdsTypeMaxSize(type)`. 
 * - `type`   : The SDS type. Can assist `sdsReqType(length)` to compute the type.
 * - `init`   : Initial string to copy, or `SDS_NOINIT` to skip initialization.
 * - `initlen`: Length of the initial string.
 * 
 * Returns:
 * - A pointer to the SDS inside `buf`. 
 */
sds sdsnewplacement(char *buf, size_t bufsize, char type, const char *init, size_t initlen) {
    assert(bufsize >= sdsReqSize(initlen, type));
    int hdrlen = sdsHdrSize(type);
    size_t usable = bufsize - hdrlen - 1;
    sds s = buf + hdrlen;
    unsigned char *fp = ((unsigned char *)s) - 1; /* flags pointer. */

    switch(type) {
        case SDS_TYPE_5: {
            *fp = type | (initlen << SDS_TYPE_BITS);
            break;
        }
        case SDS_TYPE_8: {
            SDS_HDR_VAR(8,s);
            sh->len = initlen;
            debugAssert(usable <= sdsTypeMaxSize(type));
            sh->alloc = usable;
            *fp = type;
            break;
