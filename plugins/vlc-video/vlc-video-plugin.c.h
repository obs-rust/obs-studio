#include "plugins/vlc-video/build/vlc-video.pch.h"

OBS_DECLARE_MODULE()
OBS_MODULE_USE_DEFAULT_LOCALE("vlc-video", "en-US")
MODULE_EXPORT const char *obs_module_description(void)
{
	return "VLC playlist source";
}

/* libvlc core */
LIBVLC_NEW libvlc_new_;
LIBVLC_RELEASE libvlc_release_;
LIBVLC_GET_VERSION libvlc_get_version_;
LIBVLC_CLOCK libvlc_clock_;
LIBVLC_EVENT_ATTACH libvlc_event_attach_;

/* libvlc media */
LIBVLC_MEDIA_NEW_PATH libvlc_media_new_path_;
LIBVLC_MEDIA_NEW_LOCATION libvlc_media_new_location_;
LIBVLC_MEDIA_ADD_OPTION libvlc_media_add_option_;
LIBVLC_MEDIA_RELEASE libvlc_media_release_;
LIBVLC_MEDIA_RETAIN libvlc_media_retain_;
LIBVLC_MEDIA_GET_META libvlc_media_get_meta_;
LIBVLC_MEDIA_TRACKS_GET libvlc_media_tracks_get_;
LIBVLC_MEDIA_TRACKS_RELEASE libvlc_media_tracks_release_;

/* libvlc media player */
LIBVLC_MEDIA_PLAYER_NEW libvlc_media_player_new_;
LIBVLC_MEDIA_PLAYER_NEW_FROM_MEDIA libvlc_media_player_new_from_media_;
LIBVLC_MEDIA_PLAYER_RELEASE libvlc_media_player_release_;
LIBVLC_VIDEO_SET_CALLBACKS libvlc_video_set_callbacks_;
LIBVLC_VIDEO_SET_FORMAT_CALLBACKS libvlc_video_set_format_callbacks_;
LIBVLC_AUDIO_SET_CALLBACKS libvlc_audio_set_callbacks_;
LIBVLC_AUDIO_SET_FORMAT_CALLBACKS libvlc_audio_set_format_callbacks_;
LIBVLC_MEDIA_PLAYER_PLAY libvlc_media_player_play_;
LIBVLC_MEDIA_PLAYER_STOP libvlc_media_player_stop_;
LIBVLC_MEDIA_PLAYER_GET_TIME libvlc_media_player_get_time_;
LIBVLC_MEDIA_PLAYER_SET_TIME libvlc_media_player_set_time_;
LIBVLC_VIDEO_GET_SIZE libvlc_video_get_size_;
LIBVLC_MEDIA_PLAYER_EVENT_MANAGER libvlc_media_player_event_manager_;
LIBVLC_MEDIA_PLAYER_GET_STATE libvlc_media_player_get_state_;
LIBVLC_MEDIA_PLAYER_GET_LENGTH libvlc_media_player_get_length_;
LIBVLC_MEDIA_PLAYER_GET_MEDIA libvlc_media_player_get_media_;

/* libvlc media list */
LIBVLC_MEDIA_LIST_NEW libvlc_media_list_new_;
LIBVLC_MEDIA_LIST_RELEASE libvlc_media_list_release_;
LIBVLC_MEDIA_LIST_ADD_MEDIA libvlc_media_list_add_media_;
LIBVLC_MEDIA_LIST_LOCK libvlc_media_list_lock_;
LIBVLC_MEDIA_LIST_UNLOCK libvlc_media_list_unlock_;
LIBVLC_MEDIA_LIST_EVENT_MANAGER libvlc_media_list_event_manager_;

/* libvlc media list player */
LIBVLC_MEDIA_LIST_PLAYER_NEW libvlc_media_list_player_new_;
LIBVLC_MEDIA_LIST_PLAYER_RELEASE libvlc_media_list_player_release_;
LIBVLC_MEDIA_LIST_PLAYER_PLAY libvlc_media_list_player_play_;
LIBVLC_MEDIA_LIST_PLAYER_PAUSE libvlc_media_list_player_pause_;
LIBVLC_MEDIA_LIST_PLAYER_STOP libvlc_media_list_player_stop_;
LIBVLC_MEDIA_LIST_PLAYER_SET_MEDIA_PLAYER
libvlc_media_list_player_set_media_player_;
LIBVLC_MEDIA_LIST_PLAYER_SET_MEDIA_LIST libvlc_media_list_player_set_media_list_;
LIBVLC_MEDIA_LIST_PLAYER_EVENT_MANAGER libvlc_media_list_player_event_manager_;
LIBVLC_MEDIA_LIST_PLAYER_SET_PLAYBACK_MODE
libvlc_media_list_player_set_playback_mode_;
LIBVLC_MEDIA_LIST_PLAYER_NEXT libvlc_media_list_player_next_;
LIBVLC_MEDIA_LIST_PLAYER_PREVIOUS libvlc_media_list_player_previous_;

void *libvlc_module = NULL;

libvlc_instance_t *libvlc = NULL;
uint64_t time_start = 0;

static bool load_vlc_funcs(void)
{
#define LOAD_VLC_FUNC(func)                                     \
	do {                                                    \
		union {                                         \
			void *obj;                              \
			__typeof__(func##_) fn;                 \
		} _sym;                                         \
		_sym.obj = os_dlsym(libvlc_module, #func);      \
		func##_ = _sym.fn;                              \
		if (!func##_) {                                 \
			blog(LOG_WARNING,                       \
			     "[vlc-video]: Could not func VLC " \
			     "function %s, VLC loading failed", \
			     #func);                            \
			return false;                           \
		}                                               \
	} while (false)

	/* libvlc core */
	LOAD_VLC_FUNC(libvlc_new);
	LOAD_VLC_FUNC(libvlc_release);
	LOAD_VLC_FUNC(libvlc_get_version);
	LOAD_VLC_FUNC(libvlc_clock);
	LOAD_VLC_FUNC(libvlc_event_attach);

	/* libvlc media */
	LOAD_VLC_FUNC(libvlc_media_new_path);
	LOAD_VLC_FUNC(libvlc_media_new_location);
	LOAD_VLC_FUNC(libvlc_media_add_option);
	LOAD_VLC_FUNC(libvlc_media_release);
	LOAD_VLC_FUNC(libvlc_media_retain);
	LOAD_VLC_FUNC(libvlc_media_get_meta);
	LOAD_VLC_FUNC(libvlc_media_tracks_get);
	LOAD_VLC_FUNC(libvlc_media_tracks_release);

	/* libvlc media player */
	LOAD_VLC_FUNC(libvlc_media_player_new);
	LOAD_VLC_FUNC(libvlc_media_player_new_from_media);
	LOAD_VLC_FUNC(libvlc_media_player_release);
	LOAD_VLC_FUNC(libvlc_video_set_callbacks);
	LOAD_VLC_FUNC(libvlc_video_set_format_callbacks);
	LOAD_VLC_FUNC(libvlc_audio_set_callbacks);
	LOAD_VLC_FUNC(libvlc_audio_set_format_callbacks);
	LOAD_VLC_FUNC(libvlc_media_player_play);
	LOAD_VLC_FUNC(libvlc_media_player_stop);
	LOAD_VLC_FUNC(libvlc_media_player_get_time);
	LOAD_VLC_FUNC(libvlc_media_player_set_time);
	LOAD_VLC_FUNC(libvlc_video_get_size);
	LOAD_VLC_FUNC(libvlc_media_player_event_manager);
	LOAD_VLC_FUNC(libvlc_media_player_get_state);
	LOAD_VLC_FUNC(libvlc_media_player_get_length);
	LOAD_VLC_FUNC(libvlc_media_player_get_media);

	/* libvlc media list */
	LOAD_VLC_FUNC(libvlc_media_list_new);
	LOAD_VLC_FUNC(libvlc_media_list_release);
	LOAD_VLC_FUNC(libvlc_media_list_add_media);
	LOAD_VLC_FUNC(libvlc_media_list_lock);
	LOAD_VLC_FUNC(libvlc_media_list_unlock);
	LOAD_VLC_FUNC(libvlc_media_list_event_manager);

	/* libvlc media list player */
	LOAD_VLC_FUNC(libvlc_media_list_player_new);
	LOAD_VLC_FUNC(libvlc_media_list_player_release);
	LOAD_VLC_FUNC(libvlc_media_list_player_play);
	LOAD_VLC_FUNC(libvlc_media_list_player_pause);
	LOAD_VLC_FUNC(libvlc_media_list_player_stop);
	LOAD_VLC_FUNC(libvlc_media_list_player_set_media_player);
	LOAD_VLC_FUNC(libvlc_media_list_player_set_media_list);
	LOAD_VLC_FUNC(libvlc_media_list_player_event_manager);
	LOAD_VLC_FUNC(libvlc_media_list_player_set_playback_mode);
	LOAD_VLC_FUNC(libvlc_media_list_player_next);
	LOAD_VLC_FUNC(libvlc_media_list_player_previous);
	return true;
}

static bool load_libvlc_module(void)
{
	libvlc_module = vlc_host_open_libvlc();
	return libvlc_module != NULL;
}

extern struct obs_source_info vlc_source_info;

bool load_libvlc(void)
{
	if (libvlc)
		return true;

	libvlc = libvlc_new_(0, 0);
	if (!libvlc) {
		blog(LOG_INFO, "[vlc-video]: Couldn't create libvlc instance");
		return false;
	}

	time_start = (uint64_t)libvlc_clock_() * 1000ULL;
	return true;
}

bool obs_module_load(void)
{
	if (!load_libvlc_module()) {
		blog(LOG_INFO, "[vlc-video]: Couldn't find VLC installation, "
			       "VLC video source disabled");
		return true;
	}

	if (!load_vlc_funcs())
		return true;

	blog(LOG_INFO, "[vlc-video]: VLC %s found, VLC video source enabled", libvlc_get_version_());

	obs_register_source(&vlc_source_info);
	return true;
}

void obs_module_unload(void)
{
	if (libvlc)
		libvlc_release_(libvlc);
	vlc_host_unload();
	if (libvlc_module)
		os_dlclose(libvlc_module);
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef LOAD_VLC_FUNC
