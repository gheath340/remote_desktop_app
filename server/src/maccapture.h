// #pragma once
// #include <stdint.h>

// // BGRA frame pointer with row stride (bytes_per_row)
// typedef void (*sck_frame_cb)(const uint8_t* data,
//                              uint32_t width,
//                              uint32_t height,
//                              uint32_t bytes_per_row);

// // Starts ScreenCaptureKit on the main display and calls cb for each frame.
// // NOTE: The pointer is only valid during the callback. Copy immediately.
// void sck_start_capture(sck_frame_cb cb);

#pragma once
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Function pointer type for sending frames to Rust
typedef void (*sck_frame_cb)(const uint8_t *data,
                             uint32_t width,
                             uint32_t height,
                             uint32_t bytes_per_row);

// Called from Rust to start capture
void sck_start_capture(sck_frame_cb cb);

#ifdef __cplusplus
}
#endif