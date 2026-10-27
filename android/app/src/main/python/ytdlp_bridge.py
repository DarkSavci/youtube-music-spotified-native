"""yt-dlp for the core.

The core gives the arguments it would give the yt-dlp program, as a JSON
array, and reads back the JSON that program would print. Run in-process
through yt-dlp's own option parser rather than its main(), which would exit
the interpreter and print to a stdout the app does not read.
"""

import json
import os
import tempfile
import threading

import yt_dlp
from yt_dlp.extractor.youtube.jsc._builtin.ejs import EJSBaseJCP

VERBOSE = False
CACHE_DIR = None

# How many preprocessed players to keep. YouTube moves to a new player every
# few days, and each is a few megabytes; the newest is the one in use.
KEEP_PLAYERS = 3

"""
Keeping the player QuickJS has already taken apart.

Solving a challenge means parsing YouTube's player, about 2.6 MB of
JavaScript, before running a few lines of it. Deno does that in well under
a second; QuickJS took about six on the emulator, every resolve. yt-dlp can
save the parsed player and hand it back next time, but leaves that switched
off because it never deletes the old ones. Here it is switched on and the
old ones are deleted when the app starts.
"""
EJSBaseJCP._ENABLE_PREPROCESSED_PLAYER_CACHE = True


def configure(temp_dir, cache_dir, verbose=False):
    global VERBOSE, CACHE_DIR
    VERBOSE = verbose
    CACHE_DIR = cache_dir
    # QuickJS runs each challenge script from a temporary file, and Android
    # has no /tmp: the app's cache directory stands in.
    os.environ["TMPDIR"] = temp_dir
    tempfile.tempdir = temp_dir
    _prune_players(os.path.join(cache_dir, EJSBaseJCP._CACHE_SECTION))


def _prune_players(section_dir):
    try:
        players = [
            os.path.join(section_dir, name)
            for name in os.listdir(section_dir)
            if name.startswith("player")
        ]
    except OSError:
        return
    players.sort(key=os.path.getmtime, reverse=True)
    for stale in players[KEEP_PLAYERS:]:
        try:
            os.remove(stale)
        except OSError:
            pass


"""
Keeping yt-dlp itself between resolves.

A YoutubeDL holds what it has fetched for as long as it lives: YouTube's
player and the iframe API among it. A new one per resolve fetched both
again every time, about a second and a half. Instances are kept per set of
options (the cookies and the client asked for differ) and lent to one
resolve at a time, since a YoutubeDL is not safe to share between threads;
the core resolves several songs at once when it prefetches.
"""
_idle = {}
_idle_lock = threading.Lock()


def _borrow(key, opts):
    with _idle_lock:
        free = _idle.get(key)
        if free:
            return free.pop()
    return yt_dlp.YoutubeDL(opts)


def _give_back(key, ydl):
    with _idle_lock:
        _idle.setdefault(key, []).append(ydl)


def run(args):
    argv = json.loads(args)
    # The core's probe for whether yt-dlp can be run at all.
    if argv == ["--version"]:
        return yt_dlp.version.__version__
    parsed = yt_dlp.parse_options(argv)
    opts = dict(parsed.ydl_opts, quiet=True, no_warnings=True, cachedir=CACHE_DIR)
    if VERBOSE:
        opts.update(quiet=False, no_warnings=False, verbose=True)
    # Everything but the address: the same options for every song.
    key = json.dumps(argv[:-1])
    ydl = _borrow(key, opts)
    try:
        info = ydl.extract_info(parsed.urls[0], download=False)
        result = json.dumps(ydl.sanitize_info(info))
    except BaseException:
        # One that failed part-way is not trusted with the next song.
        ydl.close()
        raise
    _give_back(key, ydl)
    return result
