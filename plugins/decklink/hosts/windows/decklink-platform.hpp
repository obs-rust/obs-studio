#pragma once

#include <combaseapi.h>
#include <DeckLinkAPI.h>
#include <DeckLinkAPIVersion.h>
typedef BOOL decklink_bool_t;
typedef BSTR decklink_string_t;
IDeckLinkDiscovery *CreateDeckLinkDiscoveryInstance(void);
IDeckLinkIterator *CreateDeckLinkIteratorInstance(void);
IDeckLinkVideoConversion *CreateVideoConversionInstance(void);
#define IUnknownUUID IID_IUnknown
typedef REFIID CFUUIDBytes;
#define CFUUIDGetUUIDBytes(x) x
