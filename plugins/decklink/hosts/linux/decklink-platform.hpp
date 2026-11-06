#pragma once

#if defined(__linux__)
#include <DeckLinkAPI.h>
#include <DeckLinkAPIVersion.h>
typedef bool decklink_bool_t;
typedef const char *decklink_string_t;
#endif
