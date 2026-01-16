/*
 * Simple C API wrapper for mdflib
 * This provides a minimal C interface for reading CAN frames from MDF4 files
 * to be used from Rust via FFI.
 */

#ifndef MDF_C_API_H
#define MDF_C_API_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

// Opaque handle types
typedef void* MdfReaderHandle;
typedef void* MdfIteratorHandle;

// CAN frame structure matching Rust's CanFrame
typedef struct {
    uint64_t timestamp_ns;    // Timestamp in nanoseconds since epoch
    uint8_t channel;          // CAN channel number
    uint32_t can_id;          // CAN message ID (11-bit or 29-bit)
    uint8_t data[64];         // Frame data (up to 64 bytes for CAN-FD)
    uint8_t data_length;      // Actual number of data bytes
    uint8_t is_extended;      // 1 if extended (29-bit) CAN ID
    uint8_t is_fd;            // 1 if CAN-FD frame
    uint8_t is_error_frame;   // 1 if error frame
    uint8_t is_remote_frame;  // 1 if remote frame
} MdfCanFrame;

// Error codes
typedef enum {
    MDF_OK = 0,
    MDF_ERROR_OPEN_FAILED = 1,
    MDF_ERROR_NOT_MDF_FILE = 2,
    MDF_ERROR_READ_FAILED = 3,
    MDF_ERROR_NO_CAN_DATA = 4,
    MDF_ERROR_NULL_HANDLE = 5,
    MDF_ERROR_END_OF_DATA = 6,
} MdfError;

// Open an MDF4 file and return a reader handle
// Returns NULL on error
MdfReaderHandle mdf_open(const char* filename, MdfError* error);

// Close the reader and free resources
void mdf_close(MdfReaderHandle reader);

// Create an iterator for CAN frames in the file
// Returns NULL if no CAN data found
MdfIteratorHandle mdf_create_can_iterator(MdfReaderHandle reader, MdfError* error);

// Get the next CAN frame from the iterator
// Returns MDF_OK if frame was read successfully
// Returns MDF_ERROR_END_OF_DATA when no more frames
MdfError mdf_iterator_next(MdfIteratorHandle iterator, MdfCanFrame* frame);

// Free the iterator
void mdf_iterator_free(MdfIteratorHandle iterator);

// Get last error message (returns pointer to static buffer)
const char* mdf_get_error_message();

#ifdef __cplusplus
}
#endif

#endif // MDF_C_API_H
