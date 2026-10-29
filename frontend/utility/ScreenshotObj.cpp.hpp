/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>
    Copyright (C) 2025 by Taylor Giampaolo <warchamp7@obsproject.com>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#include "frontend/build/frontend-utility-screenshotobj.pch.hpp"

ScreenshotObj::ScreenshotObj(obs_source_t *source, const Options &options)
	: weakSource(OBSGetWeakRef(source)),
	  options(options)
{
	obs_add_tick_callback(renderTick, this);
}

ScreenshotObj::ScreenshotObj(obs_source_t *source) : ScreenshotObj(source, {}) {}

ScreenshotObj::~ScreenshotObj()
{
	obs_enter_graphics();
	gs_stagesurface_destroy(stagesurf);
	gs_texrender_destroy(texrender);
	obs_leave_graphics();

	obs_remove_tick_callback(renderTick, this);
}

void ScreenshotObj::renderTick(void *param, float)
{
	ScreenshotObj *self = static_cast<ScreenshotObj *>(param);
	if (self->stage() == ScreenshotObj::Stage::Finished) {
		return;
	}

	obs_enter_graphics();
	self->processStage();
	obs_leave_graphics();
}

void ScreenshotObj::renderScreenshot()
{
	OBSSourceAutoRelease source = OBSGetStrongRef(weakSource);

	if (source) {
		sourceWidth = obs_source_get_width(source);
		sourceHeight = obs_source_get_height(source);
	} else {
		obs_video_info ovi;
		obs_get_video_info(&ovi);
		sourceWidth = ovi.base_width;
		sourceHeight = ovi.base_height;
	}

	if (!sourceWidth || !sourceHeight) {
		blog(LOG_WARNING, "Cannot render source, invalid target size");
		obs_remove_tick_callback(renderTick, this);
		deleteLater();
		return;
	}

	const enum gs_color_space space = ScreenshotColorSpace(source);
	const enum gs_color_format format = gs_get_format_from_space(space);

	outputWidth = options.size.isValid() ? static_cast<uint32_t>(options.size.width()) : sourceWidth;
	outputHeight = options.size.isValid() ? static_cast<uint32_t>(options.size.height()) : sourceHeight;

	texrender = gs_texrender_create(format, GS_ZS_NONE);
	stagesurf = gs_stagesurface_create(outputWidth, outputHeight, format);

	RenderScreenshot(texrender, source, sourceWidth, sourceHeight, outputWidth, outputHeight, space);
}

void ScreenshotObj::processStage()
{
	switch (stage_) {
	case Stage::Render:
		renderScreenshot();
		stage_ = Stage::Download;
		break;
	case Stage::Download:
		downloadData();
		stage_ = Stage::Output;
		break;
	case Stage::Output:
		copyData();
		QMetaObject::invokeMethod(this, &ScreenshotObj::handleSave, Qt::QueuedConnection);
		obs_remove_tick_callback(renderTick, this);
		stage_ = Stage::Finished;
		break;
	case Stage::Finished:
		break;
	}
}

void ScreenshotObj::downloadData()
{
	gs_stage_texture(stagesurf, gs_texrender_get_texture(texrender));
}

void ScreenshotObj::copyData()
{
	uint8_t *videoData = nullptr;
	uint32_t videoLinesize = 0;

	if (gs_stagesurface_map(stagesurf, &videoData, &videoLinesize)) {
		if (gs_stagesurface_get_color_format(stagesurf) == GS_RGBA16F) {
			const uint32_t linesize = outputWidth * 8;
			half_bytes.reserve(outputWidth * outputHeight * 8);

			for (uint32_t y = 0; y < outputHeight; ++y) {
				const uint8_t *const line = videoData + (y * videoLinesize);
				half_bytes.insert(half_bytes.end(), line, line + linesize);
			}
		} else {
			image = QImage(static_cast<int>(outputWidth), static_cast<int>(outputHeight),
				       QImage::Format::Format_RGBX8888);

			const size_t linesize = static_cast<size_t>(image.bytesPerLine());
			for (uint32_t y = 0; y < outputHeight; ++y) {
				memcpy(image.scanLine(static_cast<int>(y)), videoData + (y * videoLinesize), linesize);
			}
		}

		gs_stagesurface_unmap(stagesurf);
	}
}

void ScreenshotObj::saveToFile()
{
	if (!options.outputToFile) {
		QMetaObject::invokeMethod(this, &ScreenshotObj::onFinished, Qt::QueuedConnection);
		return;
	}

	OBSBasic *main = OBSBasic::Get();
	config_t *config = main->Config();

	const char *mode = config_get_string(config, "Output", "Mode");
	const char *type = config_get_string(config, "AdvOut", "RecType");
	const char *adv_path = strcmp(type, "Standard") ? config_get_string(config, "AdvOut", "FFFilePath")
							: config_get_string(config, "AdvOut", "RecFilePath");
	const char *rec_path = strcmp(mode, "Advanced") ? config_get_string(config, "SimpleOutput", "FilePath")
							: adv_path;

	bool noSpace = config_get_bool(config, "SimpleOutput", "FileNameWithoutSpace");
	const char *filenameFormat = config_get_string(config, "Output", "FilenameFormatting");
	bool overwriteIfExists = config_get_bool(config, "Output", "OverwriteIfExists");

	const char *ext = half_bytes.empty() ? "png" : "jxr";
	path = GetOutputFilename(rec_path, ext, noSpace, overwriteIfExists,
				 GetFormatString(filenameFormat, "Screenshot", nullptr).c_str());

	thread = std::thread([this] {
		muxFile();
		QMetaObject::invokeMethod(this, &ScreenshotObj::onFinished, Qt::QueuedConnection);
	});
}

void ScreenshotObj::muxFile()
{
	/* GetOutputFilename() created the file empty to reserve its name;
	 * remove it again if nothing could be saved into it. */
	if (half_bytes.empty()) {
		if (!image.save(QT_UTF8(path.c_str()))) {
			output_filename_release(path.c_str());
		}
		blog(LOG_INFO, "Saved screenshot to '%s'", path.c_str());
	} else {
		if (!SaveHdrScreenshot(path.c_str(), half_bytes.data(), outputWidth, outputHeight)) {
			output_filename_release(path.c_str());
		}
	}
}

void ScreenshotObj::onFinished()
{
	if (thread.joinable()) {
		thread.join();
	}

	if (outputWidth > 0 && outputHeight > 0) {
		if (options.outputToFile) {
			emit imageSaved(path);
		}

		emit imageReady(image.copy());
	}

	this->deleteLater();
}

void ScreenshotObj::handleSave()
{
	saveToFile();
}
