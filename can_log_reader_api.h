/**
 * CAN Log Reader - C Callback API
 *
 * This header file defines the C API for user callbacks.
 * Users can write C code that implements these functions and compile to a DLL/SO.
 *
 * Implementation: Phase 11
 */

#ifndef CAN_LOG_READER_API_H
#define CAN_LOG_READER_API_H

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Signal callback context
 * Provides information about the current signal change
 */
typedef struct {
    const char* signal_name;
    const char* message_name;
    uint32_t can_id;
    uint8_t channel;
    const char* sender;

    // Value information
    double current_value;
    double previous_value;

    // Timing information
    uint64_t timestamp_ns;        // Absolute timestamp in nanoseconds
    uint64_t delta_from_start_ns; // Delta from log start
    uint64_t delta_from_prev_ns;  // Delta from previous change
} SignalCallbackContext;

/**
 * CAN-TP callback context
 * Provides information about a reconstructed CAN-TP message
 */
typedef struct {
    uint32_t source_addr;
    uint32_t target_addr;
    uint8_t channel;
    const uint8_t* payload;
    size_t payload_length;
    uint64_t timestamp_ns;
} CanTpCallbackContext;

/**
 * API functions callable from user callbacks
 */

/** Append a message to the RAW section of the report */
void append_to_raw(const char* message);

/** Programmatically start an event */
void start_event(const char* event_name);

/** Programmatically stop an event */
void stop_event(const char* event_name);

/** Trigger an event error with a reason */
void trigger_event_error(const char* event_name, const char* reason);

/** Get the previous value of a signal */
double get_prev_value(const char* signal_name);

/**
 * User-implemented callback functions
 * The DLL/SO must export these functions (or a subset of them)
 */

/**
 * Signal callback - called when a tracked signal changes
 *
 * @param ctx Signal callback context
 * @return true to continue processing, false to stop
 */
typedef bool (*signal_callback_fn)(const SignalCallbackContext* ctx);

/**
 * CAN-TP callback - called when a CAN-TP message is reconstructed
 *
 * @param ctx CAN-TP callback context
 * @return true to continue processing, false to stop
 */
typedef bool (*cantp_callback_fn)(const CanTpCallbackContext* ctx);

/**
 * Example user callback implementations
 * (These are examples - actual implementation will vary)
 */

// Example: Handle diagnostic messages
// bool handle_diagnostic_request(const SignalCallbackContext* ctx);

// Example: Parse UDS messages
// bool parse_uds_message(const CanTpCallbackContext* ctx);

#ifdef __cplusplus
}
#endif

#endif /* CAN_LOG_READER_API_H */
