/* These return 0 on success, 1 on failure.  The parsed array, if not empty, is malloc'd. */
int parse_longs(const char* input, long** parsed, size_t* num_parsed);
int parse_int(const char* input, int* n);
