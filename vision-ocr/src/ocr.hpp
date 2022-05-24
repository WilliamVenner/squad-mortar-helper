#include <tesseract/baseapi.h>

extern "C" struct OcrResult {
	const char* text;
	float confidence;
	int x1, y1, x2, y2;
};

extern "C" typedef void (*IterFn)(void* const, OcrResult);

extern "C" const char* const smh_ocr_tesseract_version();
extern "C" int smh_ocr_init(tesseract::TessBaseAPI** const out, const char* data, const int len, const char* lang);
extern "C" void smh_ocr_destroy(tesseract::TessBaseAPI* const tesseract);
extern "C" void smh_ocr_recognise(tesseract::TessBaseAPI* const tesseract, const int ppi, const unsigned char* image, const int width, const int height, const int bytes_per_pixel, const int bytes_per_line);
extern "C" void smh_ocr_iter(tesseract::TessBaseAPI* const tesseract, void* const state, const IterFn iter_fn);