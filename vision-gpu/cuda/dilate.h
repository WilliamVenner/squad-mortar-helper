#include <stdint.h>

#include <npp.h>
#include <nppdefs.h>
#include <nppcore.h>
#include <nppi.h>
#include <npps.h>

extern "C" int gpu_dilate(
	const uint8_t* const input,
	uint8_t* const output,
	const uint32_t w,
	const uint32_t h,
	const uint8_t* const kernel,
	const uint32_t kernel_w,
	const uint32_t kernel_h
);