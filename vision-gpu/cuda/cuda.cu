// TODO can we replace some DeviceBox<T> with __device__ variables here?

#ifndef SMH_CONSTS
#include "../../vision-common/src/consts/consts.cu"
#endif

#include <math_constants.h>

#if __CUDA_ARCH__ < 600
#define atomicMax_block atomicMax
#define atomicMin_block atomicMin
#define atomicExch_block atomicExch
#define atomicAdd_block atomicAdd
#define atomicCAS_block atomicCAS
#endif

#define clamp(x, a, b) max((a), min((b), (x)))

#define XY(x, y) const uint32_t x, const uint32_t y
#define WH(w, h) const uint32_t w, const uint32_t h
#define XYWH(x, y, w, h) XY(x, y), WH(w, h)

#define LUMA_R 0.2126f
#define LUMA_G 0.7152f
#define LUMA_B 0.0722f
#define RGB8_TO_LUMA8(r, g, b) (uint8_t)(float(r) * LUMA_R + float(g) * LUMA_G + float(b) * LUMA_B)

#define PACK_1616_INTO_32(x, y) ((uint32_t)((uint32_t)(y) << 16) | (uint32_t)(x))
#define UNPACK_X_1616_FROM_32(xy) (uint16_t)((xy)&0xFFFF)
#define UNPACK_Y_1616_FROM_32(xy) (uint16_t)(((xy) >> 16) & 0xFFFF)

#define PACK_3232_INTO_64(x, y) ((uint64_t)((uint64_t)(y) << 32) | (uint64_t)(x))
#define UNPACK_X_3232_FROM_64(xy) (uint32_t)((xy)&0xFFFFFFFF)
#define UNPACK_Y_3232_FROM_64(xy) (uint32_t)(((xy) >> 32) & 0xFFFFFFFF)

#define IS_FIRST_THREAD threadIdx.x == 0 && threadIdx.y == 0 && threadIdx.z == 0 && blockIdx.x == 0 && blockIdx.y == 0 && blockIdx.z == 0

#define INT_DIV_CEIL(self, rhs) (self + rhs - 1) / rhs

#ifdef ZERO_DEBUG
#define IS_NOT_ZERO(x) (x == 255)
#else
#define IS_NOT_ZERO(x) (x != 0)
#endif

__device__ __forceinline__ float atomicMax_block(float *const addr, const float value)
{
	float old;

	old = (value >= 0) ? __int_as_float(atomicMax_block((int *)addr, __float_as_int(value))) : __uint_as_float(atomicMin_block((unsigned int *)addr, __float_as_uint(value)));

	return old;
}

__device__ __forceinline__ float atomicCAS_block(float *const addr, const float compare, const float val)
{
	return __int_as_float(atomicCAS_block((int *const)addr, __float_as_int(compare), __float_as_int(val)));
}

__device__ uint8_t sat_subu8b(const uint8_t x, const uint8_t y)
{
	uint8_t res = x - y;
	res &= -(res <= x);

	return res;
}

__device__ uint8_t sat_addu8b(const uint8_t x, const uint8_t y)
{
	uint8_t res = x + y;
	res |= -(res < x);

	return res;
}

__device__ uint32_t sat_addu32b(const uint32_t x, const uint32_t y)
{
	uint32_t res = x + y;
	res |= -(res < x);

	return res;
}

__device__ uint16_t sat_subu16b(const uint16_t x, const uint16_t y)
{
	uint16_t res = x - y;
	res &= -(res <= x);

	return res;
}

extern "C" class HSV
{
public:
	// [0..360]
	uint16_t h;

	// [0..100]
	uint8_t s;

	// [0..100]
	uint8_t v;
};

extern "C" __align__(1) class RGB
{
public:
	uint8_t r, g, b;

	__device__ RGB() : r(0), g(0), b(0) {}
	__device__ RGB(const uint8_t r, const uint8_t g, const uint8_t b) : r(r), g(g), b(b) {}

	__device__ uint8_t operator[](uint8_t i) const
	{
#ifndef NDEBUG
		if (i > 2) [[unlikely]]
			return 0;
#endif
		return ((uint8_t *)this)[i];
	}

	__device__ uint8_t luma8() const
	{
		return RGB8_TO_LUMA8(r, g, b);
	}

	__device__ uint8_t mean() const
	{
		return (uint8_t)(((float)r + (float)g + (float)b) / 3.f);
	}

	static __device__ RGB from_hsv(HSV hsv)
	{
		float r, g, b;
		float h, s, v;

		h = (float)hsv.h;
		s = (float)hsv.s / 100.f;
		v = (float)hsv.v / 100.f;

		float f = h / 60.0f;
		float hi = floorf(f);
		f = f - hi;
		float p = v * (1.f - s);
		float q = v * (1.f - s * f);
		float t = v * (1.f - s * (1.f - f));

		if (hi == 0.0f || hi == 6.0f)
		{
			r = v;
			g = t;
			b = p;
		}
		else if (hi == 1.0f)
		{
			r = q;
			g = v;
			b = p;
		}
		else if (hi == 2.0f)
		{
			r = p;
			g = v;
			b = t;
		}
		else if (hi == 3.0f)
		{
			r = p;
			g = q;
			b = v;
		}
		else if (hi == 4.0f)
		{
			r = t;
			g = p;
			b = v;
		}
		else
		{
			r = v;
			g = p;
			b = q;
		}

		uint8_t red = (uint8_t)__float2uint_rn(255.0f * r);
		uint8_t green = (uint8_t)__float2uint_rn(255.0f * g);
		uint8_t blue = (uint8_t)__float2uint_rn(255.0f * b);
		return RGB{red, green, blue};
	}

	__device__ HSV to_hsv() const
	{
		float r, g, b;
		float h, s, v;

		r = this->r / 255.0f;
		g = this->g / 255.0f;
		b = this->b / 255.0f;

		float max = fmax(r, fmax(g, b));
		float min = fmin(r, fmin(g, b));
		float diff = max - min;

		v = max;

		if (v == 0.0f)
		{ // black
			h = s = 0.0f;
		}
		else
		{
			s = diff / v;
			if (diff < 0.001f)
			{ // grey
				h = 0.0f;
			}
			else
			{ // color
				if (max == r)
				{
					h = 60.0f * (g - b) / diff;
					if (h < 0.0f)
					{
						h += 360.0f;
					}
				}
				else if (max == g)
				{
					h = 60.0f * (2 + (b - r) / diff);
				}
				else
				{
					h = 60.0f * (4 + (r - g) / diff);
				}
			}
		}

		return HSV{(uint16_t)h, (uint8_t)(s * 100.f), (uint8_t)(v * 100.f)};
	}
};

extern "C" __align__(1) class BGRA
{
public:
	uint8_t b, g, r, a;

	__device__ __forceinline__ RGB to_rgb() const
	{
		return RGB(r, g, b);
	}
};

extern "C" struct RGBA
{
	uint8_t r, g, b, a;
};

extern "C" struct Point
{
	float x, y;
};

extern "C" struct Line
{
	Point p0, p1;
};

namespace markers
{
	extern "C" struct TemplateMatch
	{
		uint32_t xy;
		uint16_t sad;
	};
};

__device__ __forceinline__ bool is_map_marker_color(RGB rgb)
{
	constexpr uint16_t MAP_MARKER_COLORS[3][3] = {
		*ALPHA_MARKER_COLOR_HSV,
		*BRAVO_MARKER_COLOR_HSV,
		*CHARLIE_MARKER_COLOR_HSV};

	HSV hsv = rgb.to_hsv();

	bool alpha_hue_ok = abs((int16_t)hsv.h - (int16_t)ALPHA_MARKER_COLOR_HSV[0]) <= FIND_MARKER_HSV_HUE_TOLERANCE;
	bool alpha_sat_ok = abs((int16_t)hsv.s - (int16_t)ALPHA_MARKER_COLOR_HSV[1]) <= FIND_MARKER_HSV_SAT_TOLERANCE;
	bool alpha_vib_ok = abs((int16_t)hsv.v - (int16_t)ALPHA_MARKER_COLOR_HSV[2]) <= FIND_MARKER_HSV_VIB_TOLERANCE;

	bool bravo_hue_ok = abs((int16_t)hsv.h - (int16_t)BRAVO_MARKER_COLOR_HSV[0]) <= FIND_MARKER_HSV_HUE_TOLERANCE;
	bool bravo_sat_ok = abs((int16_t)hsv.s - (int16_t)BRAVO_MARKER_COLOR_HSV[1]) <= FIND_MARKER_HSV_SAT_TOLERANCE;
	bool bravo_vib_ok = abs((int16_t)hsv.v - (int16_t)BRAVO_MARKER_COLOR_HSV[2]) <= FIND_MARKER_HSV_VIB_TOLERANCE;

	bool charlie_hue_ok = abs((int16_t)hsv.h - (int16_t)CHARLIE_MARKER_COLOR_HSV[0]) <= FIND_MARKER_HSV_HUE_TOLERANCE;
	bool charlie_sat_ok = abs((int16_t)hsv.s - (int16_t)CHARLIE_MARKER_COLOR_HSV[1]) <= FIND_MARKER_HSV_SAT_TOLERANCE;
	bool charlie_vib_ok = abs((int16_t)hsv.v - (int16_t)CHARLIE_MARKER_COLOR_HSV[2]) <= FIND_MARKER_HSV_VIB_TOLERANCE;

	return (alpha_hue_ok &&
			alpha_sat_ok &&
			alpha_vib_ok) ||
		   (bravo_hue_ok &&
			bravo_sat_ok &&
			bravo_vib_ok) ||
		   (charlie_hue_ok &&
			charlie_sat_ok &&
			charlie_vib_ok);
}

// Counts the number of red pixels where the "CLOSE DEPLOYMENT BUTTON" is on the screen
extern "C" __global__ void count_close_deployment_button_red_pixels(
	const BGRA *const input,
	const uint32_t stride,
	XYWH(btn_x, btn_y, btn_w, btn_h),
	uint32_t *const red_pixels)
{
	__shared__ uint32_t block_red_pixels;

	if (threadIdx.x == 0 && threadIdx.y == 0) [[unlikely]]
		block_red_pixels = 0;

	__threadfence_block();

	const unsigned int x = threadIdx.x + blockIdx.x * blockDim.x;
	const unsigned int y = threadIdx.y + blockIdx.y * blockDim.y;

	if (x < btn_w && y < btn_h) [[likely]]
	{
		const unsigned int btn_roi_x = x + btn_x;
		const unsigned int btn_roi_y = y + btn_y;

		const RGB px = input[btn_roi_y * stride + btn_roi_x].to_rgb();

		bool passed = true;
		for (uint8_t i = 0; i < 3; i++)
		{
			if ((uint16_t)abs(CLOSE_DEPLOYMENT_BUTTON_COLOR[i] - (int16_t)px[i]) > CLOSE_DEPLOYMENT_BUTTON_TOLERANCE)
			{
				passed = false;
			}
		}

		if (passed)
			atomicAdd_block(&block_red_pixels, 1);
	}

	__syncthreads();

	if (threadIdx.x == 0 && threadIdx.y == 0) [[unlikely]]
		atomicAdd(red_pixels, block_red_pixels);
}

extern "C" __global__ void crop_to_map_brq(
	const BGRA *const input,
	const uint32_t stride,
	XYWH(roi_x, roi_y, roi_w, roi_h),
	RGB *const output)
{
	const unsigned int x = threadIdx.x + blockIdx.x * blockDim.x;
	const unsigned int y = threadIdx.y + blockIdx.y * blockDim.y;

	if (x >= roi_w || y >= roi_h || x >= stride) [[unlikely]]
		return;

	output[y * roi_w + x] = input[(y + roi_y) * stride + (x + roi_x)].to_rgb();
}

extern "C" __global__ void crop_to_map_grayscale(
	const BGRA *const input,
	const uint32_t stride,
	XYWH(roi_x, roi_y, roi_w, roi_h),
	RGB *const output,
	RGBA *const gray_output)
{
	const unsigned int x = threadIdx.x + blockIdx.x * blockDim.x;
	const unsigned int y = threadIdx.y + blockIdx.y * blockDim.y;

	if (x >= roi_w || y >= roi_h || x >= stride) [[unlikely]]
		return;

	const RGB pixel = input[(y + roi_y) * stride + (x + roi_x)].to_rgb();
	output[y * roi_w + x] = pixel;

	const uint8_t luma8 = pixel.luma8();
	gray_output[y * roi_w + x] = RGBA{luma8, luma8, luma8, 255};
}

extern "C" __global__ void crop_to_map(
	const BGRA *const input,
	const uint32_t stride,
	XYWH(roi_x, roi_y, roi_w, roi_h),
	RGB *const output,
	RGBA *const ui_output)
{
	const unsigned int x = threadIdx.x + blockIdx.x * blockDim.x;
	const unsigned int y = threadIdx.y + blockIdx.y * blockDim.y;

	if (x >= roi_w || y >= roi_h || x >= stride) [[unlikely]]
		return;

	const RGB pixel = input[(y + roi_y) * stride + (x + roi_x)].to_rgb();
	output[y * roi_w + x] = pixel;

	ui_output[y * roi_w + x] = RGBA{pixel.r, pixel.g, pixel.b, 255};
}

// Isolate whiteish text
// We don't use binary thresholding here because the OCR reads
// antialiasing better than we can threshold it
extern "C" __global__ void ocr_preprocess(
	const RGB *const input,
	WH(w, h),
	uint8_t *const out)
{
	const unsigned int x = threadIdx.x + blockIdx.x * blockDim.x;
	const unsigned int y = threadIdx.y + blockIdx.y * blockDim.y;

	if (x >= w || y >= h) [[unlikely]]
		return;

	const RGB pixel = input[y * w + x];

	const auto ocr_monochromaticy = [](const RGB pixel)
	{
		uint16_t diff = 0;
		for (uint8_t i = 0; i < 3; i++)
		{
			for (uint8_t j = 0; j < 3; j++)
			{
				diff += abs((int16_t)pixel[i] - (int16_t)pixel[j]);
			}
		}
		return diff;
	};

	const uint16_t diff = ocr_monochromaticy(pixel);
	if (diff <= OCR_PREPROCESS_MONOCHROMATICY_THRESHOLD)
	{
		for (uint8_t i = 0; i < 3; i++)
		{
			if (pixel[i] < OCR_PREPROCESS_BRIGHTNESS_THRESHOLD)
			{
				goto edge;
			}
		}
		goto keep;
	}

edge:
	if (diff <= OCR_PREPROCESS_SIMILARITY_EDGE_THRESHOLD)
	{
		for (uint8_t i = 0; i < 3; i++)
		{
			if (pixel[i] < OCR_PREPROCESS_BRIGHTNESS_EDGE_THRESHOLD)
			{
				goto dont_keep;
			}
		}

		for (int32_t xx = x - OCR_PREPROCESS_DILATE_RADIUS; xx <= x + OCR_PREPROCESS_DILATE_RADIUS; xx++)
		{
			for (int32_t yy = y - OCR_PREPROCESS_DILATE_RADIUS; yy <= y + OCR_PREPROCESS_DILATE_RADIUS; yy++)
			{
				if (xx < 0 || xx >= w || yy < 0 || yy >= h)
					continue;

				const RGB pixel = input[yy * w + xx];

				for (uint8_t i = 0; i < 3; i++)
				{
					if (pixel[i] < OCR_PREPROCESS_BRIGHTNESS_THRESHOLD)
					{
						goto next_neighbour;
					}
				}

				if (ocr_monochromaticy(pixel) <= OCR_PREPROCESS_MONOCHROMATICY_THRESHOLD)
				{
					goto keep;
				}

			next_neighbour:
			}
		}
	}

dont_keep:
	out[y * w + x] = 255;
	return;

keep:
	out[y * w + x] = 255 - pixel.luma8();
}

extern "C" __global__ void find_scales_preprocess(
	const RGB *const input,
	WH(w, h),
	const uint32_t scales_start_y,
	uint8_t *const output)
{
	const unsigned int x = threadIdx.x + blockIdx.x * blockDim.x;
	const unsigned int y = (threadIdx.y + blockIdx.y * blockDim.y) + scales_start_y;

	if (x >= w || y >= h) [[unlikely]]
		return;

	// Only need black & white pixels
	if (IS_NOT_ZERO(input[y * w + x].luma8()))
	{
		output[y * w + x] = 255;
	}
	else
	{
		output[y * w + x] = 0;
	}
}

extern "C" __global__ void isolate_map_markers(
	RGB *const input,
	WH(w, h),

	markers::TemplateMatch *const marked_map_marker_pixels,
	uint32_t *const marked_map_marker_pixels_count,
	const uint32_t marker_size)
{
	const unsigned int x = threadIdx.x + blockIdx.x * blockDim.x;
	const unsigned int y = threadIdx.y + blockIdx.y * blockDim.y;

	if (x >= w || y >= h) [[unlikely]]
		return;

	if (!is_map_marker_color(input[y * w + x]))
	{
		input[y * w + x] = RGB(0, 0, 0);
	}
	else if (x < w - marker_size && y < h - marker_size)
	{
		marked_map_marker_pixels[atomicAdd(marked_map_marker_pixels_count, 1)] = markers::TemplateMatch{
			y * w + x,
			0};
	}
}

extern "C" __global__ void filter_map_marker_icons(
	RGB *const input,
	const uint32_t stride,

	markers::TemplateMatch *const marked_map_marker_pixels,

	const RGBA **const markers,
	const uint32_t marker_size,

	const uint32_t markers_n,
	const uint32_t marked_map_marker_pixels_count)
{
	const unsigned int x = threadIdx.x + blockIdx.x * blockDim.x;
	const unsigned int y = threadIdx.y + blockIdx.y * blockDim.y;

	if (x >= markers_n || y >= marked_map_marker_pixels_count) [[unlikely]]
		return;

	const RGBA *const marker = markers[x];

	markers::TemplateMatch &template_match = marked_map_marker_pixels[y];

	const uint32_t xx = template_match.xy % stride;
	const uint32_t yy = template_match.xy / stride;

	for (uint32_t marker_x = 0; marker_x < marker_size; marker_x++)
	{
		for (uint32_t marker_y = 0; marker_y < marker_size; marker_y++)
		{
			RGBA marker_pixel = marker[marker_y * marker_size + marker_x];
			RGB pixel = input[(yy + marker_y) * stride + (xx + marker_x)];

			uint16_t ad = (uint16_t)abs((int16_t)pixel.r - (int16_t)marker_pixel.r) + (uint16_t)abs((int16_t)pixel.g - (int16_t)marker_pixel.g) + (uint16_t)abs((int16_t)pixel.b - (int16_t)marker_pixel.b);
			ad = (float)ad * ((float)marker_pixel.a / 255.0); // alpha blending
			template_match.sad += ad;
		}
	}
}

extern "C" __global__ void filter_map_marker_icons_clear(
	RGB *const input,
	WH(w, h),

	const uint32_t min_sad_xy,
	const uint32_t map_marker_size)
{
	const uint32_t roi_x = min_sad_xy % w;
	const uint32_t roi_y = min_sad_xy / w;

	const unsigned int x = (threadIdx.x + blockIdx.x * blockDim.x) + roi_x;
	const unsigned int y = (threadIdx.y + blockIdx.y * blockDim.y) + roi_y;

	if (x >= w || y >= h) [[unlikely]]
		return;

	// Erase the marker icon from the map!
	input[y * w + x] = RGB(0, 0, 0);

	// Trick the line segment detection algorithm into continuing the line by placing a 4x4 square where the marker icon was pointing
	// It should hopefully fill the gap and continue the line
	const uint32_t sq_x = roi_x + (map_marker_size / 2);
	const uint32_t sq_y = roi_y + roundf((float)map_marker_size * MAP_MARKER_POI_LOCATION);
	if (x >= sq_x - 2 && x <= sq_x + 2 && y >= sq_y - 2 && y <= sq_y + 2)
	{
		input[y * w + x] = RGB(0, 255, 0);
	}
}

extern "C" __global__ void mask_marker_lines(
	const RGB *const input,
	WH(w, h),
	uint8_t *const output)
{
	const unsigned int x = threadIdx.x + blockIdx.x * blockDim.x;
	const unsigned int y = threadIdx.y + blockIdx.y * blockDim.y;

	if (x >= w || y >= h) [[unlikely]]
		return;

	if (is_map_marker_color(input[y * w + x]))
	{
		output[y * w + x] = 255;
	}
	else
	{
		output[y * w + x] = 0;
	}
}

extern "C" __global__ void find_longest_line(
	const uint8_t *const input,
	WH(w, h),

	const Point pt,
	const float max_gap,

	Line *const longest_lines)
{
	__shared__ float longest_line_length;
	if (threadIdx.x == 0)
	{
		longest_line_length = 0.0f;
	}

	const float theta = ((float)(threadIdx.x + blockIdx.x * blockDim.x) / 10.0) * CUDART_PI_F / 180.0;

	float x = pt.x;
	float y = pt.y;

	const float x_start = x;
	const float y_start = y;
	float x_end = x;
	float y_end = y;

	float gap = 0.0;
	float gap_x = 0.0;
	float gap_y = 0.0;

	const float dx = cosf(theta);
	const float dy = sinf(theta);
	float x_offset = 0.0;
	float y_offset = 0.0;

	while (x >= 0.0 && y >= 0.0 && x < w && y < h)
		[[likely]]
		{
			if (input[(uint32_t)y * w + (uint32_t)x] == 255)
			{
				// there's no gap, reset state
				gap = 0.0;
				gap_x = 0.0;
				gap_y = 0.0;
			}
			else if (gap >= max_gap)
			{
				// gap didn't close, abort
				// restore saved state
				x = gap_x;
				y = gap_y;
				break;
			}
			else if (gap == 0.0)
			{
				// save the state of (x, y) so we can restore it later if the gap isn't closed
				gap = 1.0;
				gap_x = x;
				gap_y = y;
			}
			else
			{
				// keep going in case there is a gap that closes
				gap += 1.0;
			}

			x_offset += dx;
			y_offset += dy;
			x = x_offset + x_start;
			y = y_offset + y_start;
		}

	if ((uint32_t)x < w && (uint32_t)y < h && input[(uint32_t)y * w + (uint32_t)x] == 0)
	{
		x_end = x - dx;
		y_end = y - dy;
	}

	const Line line = Line{
		Point{x_start, y_start},
		Point{x_end, y_end}};

	const float length = ((line.p0.x - line.p1.x) * (line.p0.x - line.p1.x)) + ((line.p0.y - line.p1.y) * (line.p0.y - line.p1.y));

	atomicMax_block(&longest_line_length, length);

	__syncthreads();

	// only one thread with the longest line can write to the output
	const bool claim = atomicCAS_block(&longest_line_length, length, -1.0) == length;
	if (claim) [[unlikely]]
	{
		longest_lines[blockIdx.x] = line;
	}
}