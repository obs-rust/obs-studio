/* The Windows startup log's security product list (#2823).
 *
 * libobs/obs-windows.c.h is compiled into this test, so the cases call
 * log_security_products_by_type() exactly as log_security_products() does,
 * with a fake IWSCProductList and fake IWscProducts that count their
 * references. log_security_products_by_type() owns the list it is given (as
 * CoCreateInstance returns it), and get_Item() hands it one reference to
 * each product. The cases pin:
 *
 *   - every path releases the list: the Initialize, get_Count and get_Item
 *     failures and the full walk;
 *   - every product get_Item() returned is released, including one whose
 *     get_ProductName or get_ProductState fails;
 *   - nothing is released twice, and the log lines are unchanged: one per
 *     product that answered both questions, none for one that didn't. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include "libobs/obs-windows.c.h"

#include <util/base.h>

/* Defined in other libobs files and not exported. */
struct obs_core *obs = NULL;

const char *get_win_release_id(void)
{
	return "test";
}

const char *obs_get_hotkey_translation(obs_key_t key, const char *def)
{
	(void)key;
	return def;
}

void initialize_crash_handler(void) {}

void *obs_graphics_thread(void *param)
{
	(void)param;
	return NULL;
}

bool obs_graphics_thread_loop(struct obs_graphics_context *context)
{
	(void)context;
	return false;
}

/* Fakes */

struct fake_product {
	IWscProduct iface;
	LONG refs;
	LONG over_released;
	HRESULT name_hr;
	HRESULT state_hr;
	const wchar_t *name;
};

#define MAX_PRODUCTS 4

struct fake_list {
	IWSCProductList iface;
	LONG refs;
	LONG over_released;
	ULONG provider;
	HRESULT init_hr;
	HRESULT count_hr;
	LONG count;
	struct fake_product *products[MAX_PRODUCTS];
};

static ULONG STDMETHODCALLTYPE product_add_ref(IWscProduct *This)
{
	struct fake_product *p = (struct fake_product *)This;
	return (ULONG)++p->refs;
}

static ULONG STDMETHODCALLTYPE product_release(IWscProduct *This)
{
	struct fake_product *p = (struct fake_product *)This;
	if (p->refs == 0) {
		p->over_released++;
		return 0;
	}
	return (ULONG)--p->refs;
}

static HRESULT STDMETHODCALLTYPE product_get_name(IWscProduct *This, BSTR *val)
{
	struct fake_product *p = (struct fake_product *)This;
	if (FAILED(p->name_hr))
		return p->name_hr;
	*val = SysAllocString(p->name);
	return *val ? S_OK : E_OUTOFMEMORY;
}

static HRESULT STDMETHODCALLTYPE product_get_state(IWscProduct *This, WSC_SECURITY_PRODUCT_STATE *val)
{
	struct fake_product *p = (struct fake_product *)This;
	if (FAILED(p->state_hr))
		return p->state_hr;
	*val = WSC_SECURITY_PRODUCT_STATE_ON;
	return S_OK;
}

static IWscProductVtbl product_vtbl = {
	.AddRef = product_add_ref,
	.Release = product_release,
	.get_ProductName = product_get_name,
	.get_ProductState = product_get_state,
};

static ULONG STDMETHODCALLTYPE list_add_ref(IWSCProductList *This)
{
	struct fake_list *l = (struct fake_list *)This;
	return (ULONG)++l->refs;
}

static ULONG STDMETHODCALLTYPE list_release(IWSCProductList *This)
{
	struct fake_list *l = (struct fake_list *)This;
	if (l->refs == 0) {
		l->over_released++;
		return 0;
	}
	return (ULONG)--l->refs;
}

static HRESULT STDMETHODCALLTYPE list_initialize(IWSCProductList *This, ULONG provider)
{
	struct fake_list *l = (struct fake_list *)This;
	l->provider = provider;
	return l->init_hr;
}

static HRESULT STDMETHODCALLTYPE list_get_count(IWSCProductList *This, LONG *val)
{
	struct fake_list *l = (struct fake_list *)This;
	if (FAILED(l->count_hr))
		return l->count_hr;
	*val = l->count;
	return S_OK;
}

static HRESULT STDMETHODCALLTYPE list_get_item(IWSCProductList *This, ULONG index, IWscProduct **val)
{
	struct fake_list *l = (struct fake_list *)This;
	if (index >= MAX_PRODUCTS || !l->products[index])
		return E_INVALIDARG;
	*val = &l->products[index]->iface;
	product_add_ref(*val);
	return S_OK;
}

static IWSCProductListVtbl list_vtbl = {
	.AddRef = list_add_ref,
	.Release = list_release,
	.Initialize = list_initialize,
	.get_Count = list_get_count,
	.get_Item = list_get_item,
};

static void init_list(struct fake_list *l)
{
	memset(l, 0, sizeof(*l));
	l->iface.lpVtbl = &list_vtbl;
	/* The reference CoCreateInstance hands to the caller. */
	l->refs = 1;
	l->init_hr = S_OK;
	l->count_hr = S_OK;
}

static void init_product(struct fake_product *p, const wchar_t *name)
{
	memset(p, 0, sizeof(*p));
	p->iface.lpVtbl = &product_vtbl;
	p->name_hr = S_OK;
	p->state_hr = S_OK;
	p->name = name;
}

/* Log capture */

static char log_text[4096];

static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)level;
	(void)param;
	size_t len = strlen(log_text);
	if (len < sizeof(log_text) - 1) {
		vsnprintf(log_text + len, sizeof(log_text) - len, format, args);
		len = strlen(log_text);
		if (len < sizeof(log_text) - 1) {
			log_text[len] = '\n';
			log_text[len + 1] = 0;
		}
	}
}

static int setup(void **state)
{
	(void)state;
	log_text[0] = 0;
	base_set_log_handler(capture_log, NULL);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	base_set_log_handler(NULL, NULL);
	return 0;
}

static void assert_list_released(const struct fake_list *l)
{
	if (l->refs != 0) {
		fprintf(stderr, "the product list still holds %ld reference(s)\n", (long)l->refs);
		fail();
	}
	assert_int_equal(l->over_released, 0);
}

static void assert_product_released(const struct fake_product *p)
{
	if (p->refs != 0) {
		fprintf(stderr, "product \"%ls\" still holds %ld reference(s)\n", p->name, (long)p->refs);
		fail();
	}
	assert_int_equal(p->over_released, 0);
}

/* Cases */

static void initialize_failure_releases_list(void **state)
{
	(void)state;
	struct fake_list l;
	init_list(&l);
	l.init_hr = E_FAIL;

	log_security_products_by_type(&l.iface, WSC_SECURITY_PROVIDER_FIREWALL);

	assert_int_equal(l.provider, WSC_SECURITY_PROVIDER_FIREWALL);
	assert_list_released(&l);
	assert_string_equal(log_text, "");
}

static void count_failure_releases_list(void **state)
{
	(void)state;
	struct fake_list l;
	init_list(&l);
	l.count_hr = E_FAIL;

	log_security_products_by_type(&l.iface, WSC_SECURITY_PROVIDER_ANTIVIRUS);

	assert_list_released(&l);
	assert_string_equal(log_text, "");
}

static void item_failure_releases_list(void **state)
{
	(void)state;
	struct fake_list l;
	init_list(&l);
	/* No product behind index 0: get_Item fails. */
	l.count = 1;

	log_security_products_by_type(&l.iface, WSC_SECURITY_PROVIDER_ANTIVIRUS);

	assert_list_released(&l);
	assert_string_equal(log_text, "");
}

static void name_failure_releases_product(void **state)
{
	(void)state;
	struct fake_list l;
	struct fake_product bad, good;
	init_list(&l);
	init_product(&bad, L"No Name AV");
	init_product(&good, L"Good AV");
	bad.name_hr = E_FAIL;
	l.count = 2;
	l.products[0] = &bad;
	l.products[1] = &good;

	log_security_products_by_type(&l.iface, WSC_SECURITY_PROVIDER_ANTIVIRUS);

	assert_product_released(&bad);
	assert_product_released(&good);
	assert_list_released(&l);
	assert_string_equal(log_text, "\tGood AV: enabled (AV)\n");
}

static void state_failure_releases_product(void **state)
{
	(void)state;
	struct fake_list l;
	struct fake_product bad, good;
	init_list(&l);
	init_product(&bad, L"No State FW");
	init_product(&good, L"Good FW");
	bad.state_hr = E_FAIL;
	l.count = 2;
	l.products[0] = &bad;
	l.products[1] = &good;

	log_security_products_by_type(&l.iface, WSC_SECURITY_PROVIDER_FIREWALL);

	assert_product_released(&bad);
	assert_product_released(&good);
	assert_list_released(&l);
	assert_string_equal(log_text, "\tGood FW: enabled (FW)\n");
}

static void full_walk_releases_everything(void **state)
{
	(void)state;
	struct fake_list l;
	struct fake_product a, b;
	init_list(&l);
	init_product(&a, L"First ASW");
	init_product(&b, L"Second ASW");
	l.count = 2;
	l.products[0] = &a;
	l.products[1] = &b;

	log_security_products_by_type(&l.iface, WSC_SECURITY_PROVIDER_ANTISPYWARE);

	assert_product_released(&a);
	assert_product_released(&b);
	assert_list_released(&l);
	assert_string_equal(log_text, "\tFirst ASW: enabled (ASW)\n\tSecond ASW: enabled (ASW)\n");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(initialize_failure_releases_list, setup, teardown),
		cmocka_unit_test_setup_teardown(count_failure_releases_list, setup, teardown),
		cmocka_unit_test_setup_teardown(item_failure_releases_list, setup, teardown),
		cmocka_unit_test_setup_teardown(name_failure_releases_product, setup, teardown),
		cmocka_unit_test_setup_teardown(state_failure_releases_product, setup, teardown),
		cmocka_unit_test_setup_teardown(full_walk_releases_everything, setup, teardown),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
