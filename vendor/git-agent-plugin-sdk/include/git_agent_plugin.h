#ifndef GIT_AGENT_PLUGIN_H
#define GIT_AGENT_PLUGIN_H
#include <stddef.h>
#include <stdint.h>

/* All pointers are valid only during invoke. Owner must release its own buffer.
 * JSON schemas are defined by crates/plugin-sdk/src/lib.rs.
 * No callbacks or plugin-owned threads may outlive invoke. */
typedef struct { uint8_t *ptr; size_t len; } GaBuffer;
typedef struct {
    uint32_t api_version;
    void *context;
    GaBuffer (*call)(void *context, const uint8_t *json, size_t len);
    void (*release_buffer)(GaBuffer buffer);
} GaHostApi;
typedef struct {
    uint32_t api_version;
    uint32_t struct_size;
    GaBuffer (*invoke)(const uint8_t *json, size_t len, GaHostApi host);
    void (*release_buffer)(GaBuffer buffer);
} GaPluginApi;

#ifdef __cplusplus
extern "C" {
#endif
/* Export this symbol; returned table is immutable for the lifetime of the DLL. */
const GaPluginApi *git_agent_plugin_entry_v1(void);
#ifdef __cplusplus
}
#endif
#endif
