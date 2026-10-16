/* Minimal VST 2.x effect with an editor, for test_vst_editor_close. It
 * passes audio through and only counts how often the host opens and closes
 * its editor (vst_test_editor_counts). */

#include <string.h>
#include "aeffectx.h"

#if defined(_WIN32)
#define VST_TEST_EXPORT extern "C" __declspec(dllexport)
#else
#define VST_TEST_EXPORT extern "C" __attribute__((visibility("default")))
#endif

struct EditorRect {
	short top, left, bottom, right;
};

static AEffect effect;
static EditorRect rect = {0, 0, 120, 200};
static int edit_opens;
static int edit_closes;

static intptr_t dispatcher(AEffect *, int opcode, int, intptr_t, void *ptr, float)
{
	switch (opcode) {
	case effEditOpen:
		edit_opens++;
		return 1;
	case effEditClose:
		edit_closes++;
		return 1;
	case effEditGetRect:
		*(EditorRect **)ptr = &rect;
		return 1;
	case effGetEffectName:
		strcpy((char *)ptr, "Editor Close Test");
		return 1;
	default:
		return 0;
	}
}

static void process_replacing(AEffect *, float **in, float **out, int frames)
{
	for (int c = 0; c < 2; c++) {
		memcpy(out[c], in[c], sizeof(float) * frames);
	}
}

static void set_parameter(AEffect *, int, float) {}

static float get_parameter(AEffect *, int)
{
	return 0.0f;
}

VST_TEST_EXPORT AEffect *VSTPluginMain(audioMasterCallback)
{
	memset(&effect, 0, sizeof(effect));
	effect.magic = kEffectMagic;
	effect.dispatcher = dispatcher;
	effect.setParameter = set_parameter;
	effect.getParameter = get_parameter;
	effect.processReplacing = process_replacing;
	effect.numInputs = 2;
	effect.numOutputs = 2;
	effect.flags = effFlagsHasEditor | effFlagsCanReplacing;
	return &effect;
}

VST_TEST_EXPORT void vst_test_editor_counts(int *opens, int *closes)
{
	*opens = edit_opens;
	*closes = edit_closes;
}
