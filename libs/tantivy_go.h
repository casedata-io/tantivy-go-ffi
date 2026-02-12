#ifndef TANTIVY_GO_H
#define TANTIVY_GO_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef void* TantivyIndexHandle;

void tantivy_free_string(char* s);
void tantivy_free_index(TantivyIndexHandle idx);

TantivyIndexHandle tantivy_create_index(const char* path, const char* schema_json, char** err);
TantivyIndexHandle tantivy_open_index(const char* path, char** err);

int32_t tantivy_add_doc(TantivyIndexHandle idx, const char* doc_json, char** err);
int32_t tantivy_commit(TantivyIndexHandle idx, char** err);
uint64_t tantivy_num_docs(TantivyIndexHandle idx);

// query_json follows the Query DSL: {"type":"text","query":"batman","limit":100}
char* tantivy_search(TantivyIndexHandle idx, const char* query_json, char** err);

#ifdef __cplusplus
}
#endif

#endif
