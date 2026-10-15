// The C++ headers go first: cmocka.h defines macros (fail, ...) that clash
// with libstdc++.
#include <chrono>
#include <string>
#include <thread>
#include <vector>

#include <QThreadPool>

#include "requesthandler/RequestBatchHandler.h"

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
// The Windows cmocka.h has no extern "C" guard of its own.
extern "C" {
#include <cmocka.h>
}

/* obs-websocket answers a RequestBatch with one result per request, and the
 * protocol layer (WebSocketServer_Protocol.cpp) labels results[i] with the
 * requestType and requestId of requests[i]. So
 * RequestBatchHandler::ProcessRequestBatch() must return the results in
 * request order. The serial modes do. With executionType Parallel, every
 * request runs as its own thread-pool task, and each task used to append its
 * result when it finished, so a slow request (a full-size screenshot) followed
 * by fast ones came back with every result attached to another request
 * (upstream obsproject/obs-websocket#1317).
 *
 * The batch handler is built from the plugin's own source. RequestHandler is
 * stubbed below: each request sleeps for its `delayMs` and answers with its
 * own requestType and `tag`, or fails when it asks to. The delays make the
 * requests finish in reverse order; with the fix the result order does not
 * depend on timing at all. Needs no libobs startup and no display. */

bool IsDebugEnabled()
{
	return false;
}

RequestHandler::RequestHandler(SessionPtr session) : _session(session) {}

RequestResult RequestHandler::ProcessRequest(const Request &request)
{
	if (request.RequestData.contains("delayMs")) {
		std::this_thread::sleep_for(std::chrono::milliseconds(request.RequestData["delayMs"].get<int>()));
	}

	if (request.RequestData.contains("fail")) {
		return RequestResult::Error(RequestStatus::ResourceNotFound, request.RequestType + " failed");
	}

	json responseData;
	responseData["requestType"] = request.RequestType;
	responseData["tag"] = request.RequestData.value("tag", -1);
	return RequestResult::Success(responseData);
}

/* Request i sleeps (count - i) * step_ms, so the last request finishes first
 * and the first one last. */
static std::vector<RequestBatchRequest> make_requests(size_t count, int step_ms,
						      RequestBatchExecutionType::RequestBatchExecutionType type)
{
	std::vector<RequestBatchRequest> requests;
	for (size_t i = 0; i < count; i++) {
		json data;
		data["tag"] = (int)i;
		data["delayMs"] = (int)(count - i) * step_ms;
		requests.emplace_back("Request" + std::to_string(i), data, type);
	}
	return requests;
}

static std::vector<RequestResult> run_batch(std::vector<RequestBatchRequest> &requests,
					    RequestBatchExecutionType::RequestBatchExecutionType type)
{
	QThreadPool pool;
	// One thread per request, so they all start at once and finish in
	// delay order.
	pool.setMaxThreadCount(requests.empty() ? 1 : (int)requests.size());
	json variables = json::object();
	std::vector<RequestResult> results =
		RequestBatchHandler::ProcessRequestBatch(pool, nullptr, type, requests, variables, false);
	pool.waitForDone();
	return results;
}

static void assert_results_in_request_order(const std::vector<RequestBatchRequest> &requests,
					    const std::vector<RequestResult> &results)
{
	assert_int_equal(results.size(), requests.size());
	for (size_t i = 0; i < requests.size(); i++) {
		if (requests[i].RequestData.contains("fail")) {
			assert_int_equal(results[i].StatusCode, RequestStatus::ResourceNotFound);
			assert_string_equal(results[i].Comment.c_str(), (requests[i].RequestType + " failed").c_str());
			continue;
		}
		assert_int_equal(results[i].StatusCode, RequestStatus::Success);
		assert_string_equal(results[i].ResponseData["requestType"].get<std::string>().c_str(),
				    requests[i].RequestType.c_str());
		assert_int_equal(results[i].ResponseData["tag"].get<int>(), (int)i);
	}
}

/* The upstream report: a slow request first, fast requests after it. */
static void test_parallel_slow_first(void **state)
{
	(void)state;
	std::vector<RequestBatchRequest> requests;
	json slow;
	slow["tag"] = 0;
	slow["delayMs"] = 300;
	requests.emplace_back("GetSourceScreenshot", slow, RequestBatchExecutionType::Parallel);
	const char *fast[] = {"GetVersion", "GetSceneList", "GetStats", "GetInputList"};
	for (int i = 0; i < 4; i++) {
		json data;
		data["tag"] = i + 1;
		requests.emplace_back(fast[i], data, RequestBatchExecutionType::Parallel);
	}

	std::vector<RequestResult> results = run_batch(requests, RequestBatchExecutionType::Parallel);
	assert_results_in_request_order(requests, results);
}

/* Eight requests that finish in reverse order. */
static void test_parallel_reverse_finish(void **state)
{
	(void)state;
	std::vector<RequestBatchRequest> requests = make_requests(8, 40, RequestBatchExecutionType::Parallel);
	std::vector<RequestResult> results = run_batch(requests, RequestBatchExecutionType::Parallel);
	assert_results_in_request_order(requests, results);
}

/* A failed request keeps its own slot; Parallel batches do not halt. */
static void test_parallel_failure_keeps_slot(void **state)
{
	(void)state;
	std::vector<RequestBatchRequest> requests = make_requests(4, 40, RequestBatchExecutionType::Parallel);
	requests[1].RequestData["fail"] = true;
	std::vector<RequestResult> results = run_batch(requests, RequestBatchExecutionType::Parallel);
	assert_results_in_request_order(requests, results);
}

/* An empty Parallel batch returns at once with no results. */
static void test_parallel_empty(void **state)
{
	(void)state;
	std::vector<RequestBatchRequest> requests;
	std::vector<RequestResult> results = run_batch(requests, RequestBatchExecutionType::Parallel);
	assert_int_equal(results.size(), 0);
}

/* Control: SerialRealtime batches already return results in request order. */
static void test_serial_realtime_order(void **state)
{
	(void)state;
	std::vector<RequestBatchRequest> requests = make_requests(4, 10, RequestBatchExecutionType::SerialRealtime);
	std::vector<RequestResult> results = run_batch(requests, RequestBatchExecutionType::SerialRealtime);
	assert_results_in_request_order(requests, results);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_parallel_slow_first),         cmocka_unit_test(test_parallel_reverse_finish),
		cmocka_unit_test(test_parallel_failure_keeps_slot), cmocka_unit_test(test_parallel_empty),
		cmocka_unit_test(test_serial_realtime_order),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
