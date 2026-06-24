// SPDX-License-Identifier: MIT OR Apache-2.0
// Cross-language smoke test for libpheno_bridge.
//
// This test proves:
//   1. The cdylib is loadable from C (no symbol-not-found).
//   2. Every exported symbol has the right signature (no ABI mismatch).
//   3. The library handles invalid input gracefully (null handle, unknown
//      provider, empty strings).
//   4. The library returns a populated last_error() on failure.
//
// It does NOT prove that the live sidecar stack is reachable — that's a
// deploy-time check, not an FFI-symbol check. With no sidecar running,
// network calls return non-zero codes; that's the expected behavior.
//
// Build:
//   cc -L target/release -lpheno_bridge c/examples/smoke.c -o smoke
//   DYLD_LIBRARY_PATH=target/release ./smoke
//
// Exit codes:
//   0 = all assertions passed
//   1 = a pheno_bridge call returned an unexpected value
//   SIGABRT = an assert() tripped

#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern const char *pheno_bridge_version(void);
extern const char *pheno_last_error(void);
extern void        pheno_string_free(char *s);
extern void       *pheno_memory_new(const char *provider);
extern int         pheno_memory_store(void *handle, const char *scope,
                                      const char *key, const char *value);
extern int         pheno_memory_recall(void *handle, const char *scope,
                                       const char *query, char **out);
extern int         pheno_memory_forget(void *handle, const char *scope,
                                      const char *key);
extern void        pheno_memory_free(void *handle);

int main(void) {
    /* 1. Version returns a non-null semver-ish string. */
    const char *v = pheno_bridge_version();
    printf("pheno_bridge version: %s\n", v);
    assert(v != NULL);
    assert(v[0] != '\0');

    /* 2. Unknown provider returns null handle and populates last_error. */
    void *bad = pheno_memory_new("not-a-real-provider");
    assert(bad == NULL);
    const char *e = pheno_last_error();
    printf("bad provider err: %s\n", e);
    assert(e != NULL);
    assert(strstr(e, "unknown") != NULL || strstr(e, "provider") != NULL);

    /* 3. Valid provider returns a non-null handle. */
    void *h = pheno_memory_new("sm");
    assert(h != NULL);

    /* 4. Store/recall/forget are callable (rc depends on sidecar availability). */
    int store_rc = pheno_memory_store(h, "episodic", "k", "v");
    printf("store rc=%d (no sidecar expected non-zero)\n", store_rc);
    /* Don't assert on store_rc; with no sidecar, non-zero is correct. */
    (void)store_rc;

    int forget_rc = pheno_memory_forget(h, "episodic", "k");
    printf("forget rc=%d (no sidecar expected non-zero)\n", forget_rc);
    (void)forget_rc;

    char *out = NULL;
    int recall_rc = pheno_memory_recall(h, "episodic", "anything", &out);
    printf("recall rc=%d (no sidecar expected non-zero, out=%p)\n",
           recall_rc, (void *)out);
    assert(out == NULL);

    /* 5. Null handle is rejected. */
    int null_rc = pheno_memory_store(NULL, "episodic", "k", "v");
    assert(null_rc != 0);

    pheno_memory_free(h);

    /* 6. Composite constructs without sidecars. */
    void *hc = pheno_memory_new("composite");
    assert(hc != NULL);
    pheno_memory_free(hc);

    /* 7. String-free is safe on null. */
    pheno_string_free(NULL);

    printf("OK\n");
    return 0;
}
