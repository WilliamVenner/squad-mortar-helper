#include <dilate.h>

extern "C" int gpu_dilate(
	const uint8_t *const input,
	uint8_t *const output,
	const uint32_t w,
	const uint32_t h,
	const uint8_t *const kernel,
	const uint32_t kernel_w,
	const uint32_t kernel_h)
{
	const int anchor_x = kernel_w / 2;
	const int anchor_y = kernel_h / 2;

	return nppiDilate_8u_C1R(
		input + (w * anchor_y) + anchor_x,
		(int)w,
		output + (w * anchor_y) + anchor_x,
		(int)w,
		NppiSize{int(w - kernel_w), int(h - kernel_h)},
		kernel,
		NppiSize{(int)kernel_w, (int)kernel_h},
		NppiPoint{anchor_x, anchor_y}
	);
}