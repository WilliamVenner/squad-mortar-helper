#include "ocr.hpp"

extern "C" const char* const smh_ocr_tesseract_version() {
	return tesseract::TessBaseAPI::Version();
}

extern "C" int smh_ocr_init(tesseract::TessBaseAPI** const out, const char* data, const int len, const char* lang) {
	tesseract::TessBaseAPI* tesseract = new tesseract::TessBaseAPI();
	*out = tesseract;

	int result = tesseract->Init(data, len, lang, tesseract::OcrEngineMode::OEM_LSTM_ONLY, nullptr, 0, nullptr, nullptr, false, nullptr);
	if (result != 0) {
		return result;
	}

	tesseract->SetPageSegMode(tesseract::PageSegMode::PSM_SPARSE_TEXT);

	return 0;
}

extern "C" void smh_ocr_destroy(tesseract::TessBaseAPI* const tesseract) {
	tesseract->End();
	delete tesseract;
}

extern "C" void smh_ocr_recognise(tesseract::TessBaseAPI* const tesseract, const int ppi, const unsigned char* image, const int width, const int height, const int bytes_per_pixel, const int bytes_per_line) {
	tesseract->SetImage(image, width, height, bytes_per_pixel, bytes_per_line);
	if (ppi > 0) tesseract->SetSourceResolution(ppi);
	tesseract->Recognize(0);
}

extern "C" void smh_ocr_iter(tesseract::TessBaseAPI* const tesseract, void* const state, const IterFn iter_fn) {
	tesseract::ResultIterator* const ri = tesseract->GetIterator();

	const tesseract::PageIteratorLevel level = tesseract::RIL_TEXTLINE;
	if (ri != 0) {
		do {
			OcrResult result = OcrResult {};

			result.text = ri->GetUTF8Text(level);

			if (!result.text) break;

			result.confidence = ri->Confidence(level);

			ri->BoundingBox(level, &result.x1, &result.y1, &result.x2, &result.y2);

			iter_fn(state, result);

			delete[] result.text;
		} while (ri->Next(level));

		delete ri;
	}
}