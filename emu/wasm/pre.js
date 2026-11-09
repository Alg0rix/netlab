// shim: emscripten 3.1.74 references __emscripten_thread_crashed from worker
// catch blocks but never emits it. The real exception is rethrown after it.
var __emscripten_thread_crashed = function () {};
