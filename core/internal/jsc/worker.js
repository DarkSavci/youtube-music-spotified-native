// The challenge solver as a process that stays up: yt-dlp-ejs's `jsc`
// (lib.min.js and core.min.js, loaded before this) behind a line protocol.
//
// One JSON object per line in, one per line out, in order:
//
//   {"op":"load","key":K,"player":SRC}         parse a player; answers
//                                              {"preprocessed":P} to keep
//   {"op":"load","key":K,"preprocessed":P}     a player parsed before
//   {"op":"solve","key":K,"n":[..],"sig":[..]} answers {"n":{c:v},"sig":{c:v}}
//
// Any failure answers {"error":"..."}. A player is parsed once, which is the
// slow part; its two functions are then kept, and a solve is two calls.
// Runs under QuickJS-NG (`qjs --std`) and deno.

const players = new Map();

// What jsc does with a preprocessed player on every call, done once: the
// preprocessed source assigns the n and sig functions to _result.
function functionsOf(preprocessed) {
  const result = { n: null, sig: null };
  Function("_result", preprocessed)(result);
  return result;
}

function handle(msg) {
  switch (msg.op) {
    case "load": {
      let preprocessed = msg.preprocessed;
      let fresh;
      if (!preprocessed) {
        const out = jsc({
          type: "player",
          player: msg.player,
          requests: [],
          output_preprocessed: true,
        });
        preprocessed = out.preprocessed_player;
        if (!preprocessed) return { error: "the solver kept no preprocessed player" };
        fresh = preprocessed;
      }
      players.set(msg.key, functionsOf(preprocessed));
      return fresh ? { preprocessed: fresh } : {};
    }
    case "solve": {
      const fns = players.get(msg.key);
      if (!fns) return { error: "player not loaded" };
      const out = {};
      for (const type of ["n", "sig"]) {
        const challenges = msg[type] || [];
        if (!challenges.length) continue;
        if (!fns[type]) return { error: `the player has no ${type} function` };
        const answers = {};
        for (const c of challenges) answers[c] = fns[type](c);
        out[type] = answers;
      }
      return out;
    }
    default:
      return { error: `unknown op ${msg.op}` };
  }
}

function answer(line) {
  let reply;
  try {
    reply = handle(JSON.parse(line));
  } catch (e) {
    reply = { error: e instanceof Error ? `${e.message}\n${e.stack}` : `${e}` };
  }
  return JSON.stringify(reply) + "\n";
}

// Inside a function: QuickJS runs this as a script, where a top-level
// `for await` would not even parse.
async function serveDeno() {
  const encoder = new TextEncoder();
  const write = (text) => {
    let bytes = encoder.encode(text);
    while (bytes.length) bytes = bytes.subarray(Deno.stdout.writeSync(bytes));
  };
  const decoder = new TextDecoder();
  let pending = "";
  for await (const chunk of Deno.stdin.readable) {
    pending += decoder.decode(chunk, { stream: true });
    let at;
    while ((at = pending.indexOf("\n")) >= 0) {
      const line = pending.slice(0, at);
      pending = pending.slice(at + 1);
      if (line) write(answer(line));
    }
  }
}

if (typeof Deno !== "undefined") {
  serveDeno();
} else {
  for (;;) {
    const line = std.in.getline();
    if (line === null) break;
    if (!line) continue;
    std.out.puts(answer(line));
    std.out.flush();
  }
}
