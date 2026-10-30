package mobile

import (
	"bytes"
	"context"
	"log/slog"
	"strings"
	"sync"
)

// LogSink receives the core's log lines, for the app to give to Android's log
// and keep in its log file. Priority is an android.util.Log level. Called from
// any goroutine.
type LogSink interface {
	Write(priority int32, line string)
}

// Android's log levels, from android.util.Log.
const (
	logVerbose int32 = 2
	logDebug   int32 = 3
	logInfo    int32 = 4
	logWarn    int32 = 5
	logError   int32 = 6
)

// SetLogSink gives the core the app's log. Called before Start; without one
// the core logs to standard error, which gomobile sends to logcat as errors
// whatever their level.
func SetLogSink(s LogSink) {
	mu.Lock()
	defer mu.Unlock()
	sink = s
}

func priorityOf(l slog.Level) int32 {
	switch {
	case l < slog.LevelDebug:
		return logVerbose
	case l < slog.LevelInfo:
		return logDebug
	case l < slog.LevelWarn:
		return logInfo
	case l < slog.LevelError:
		return logWarn
	default:
		return logError
	}
}

// sinkHandler formats a record as the text handler does, without the time and
// level the Android log already keeps, and hands it to the sink.
type sinkHandler struct {
	sink  LogSink
	mu    *sync.Mutex
	buf   *bytes.Buffer
	inner slog.Handler
}

func newSinkHandler(s LogSink, level slog.Level) *sinkHandler {
	buf := &bytes.Buffer{}
	return &sinkHandler{
		sink: s,
		mu:   &sync.Mutex{},
		buf:  buf,
		inner: slog.NewTextHandler(buf, &slog.HandlerOptions{
			Level: level,
			ReplaceAttr: func(groups []string, a slog.Attr) slog.Attr {
				if len(groups) == 0 && (a.Key == slog.TimeKey || a.Key == slog.LevelKey) {
					return slog.Attr{}
				}
				return a
			},
		}),
	}
}

func (h *sinkHandler) Enabled(ctx context.Context, l slog.Level) bool {
	return h.inner.Enabled(ctx, l)
}

func (h *sinkHandler) Handle(ctx context.Context, r slog.Record) error {
	h.mu.Lock()
	h.buf.Reset()
	err := h.inner.Handle(ctx, r)
	line := strings.TrimRight(h.buf.String(), "\n")
	h.mu.Unlock()
	if err != nil {
		return err
	}
	// Outside the lock: the app's side may itself be slow.
	h.sink.Write(priorityOf(r.Level), line)
	return nil
}

func (h *sinkHandler) WithAttrs(attrs []slog.Attr) slog.Handler {
	next := *h
	next.inner = h.inner.WithAttrs(attrs)
	return &next
}

func (h *sinkHandler) WithGroup(name string) slog.Handler {
	next := *h
	next.inner = h.inner.WithGroup(name)
	return &next
}
