You are resolving merge conflicts in a `git rebase` of a pull request onto
`origin/master` in the obs-rust/obs-studio repository. The rebase is stopped.
The conflicted files and the PR commit being replayed are listed at the end.

Do exactly this:
1. For each conflicted file, read it and resolve every conflict. "HEAD"/ours is
   current master; the other side is the PR commit. Most conflicts here are
   shared append points (unity-group includes in `libobs/build/*.h.c`,
   declarations in `obs-internal.h`, `test/cmocka/build/test-graph.json`,
   `mod` lists, `rust/README.md` table rows): keep both sides' additions, keep every line the PR deleted deleted (a port
   removes a C file's include from a core unity group when it moves the file to
   its ported group; putting the include back compiles the C twice), and
   keep sorted lists sorted. When both sides changed the same line, combine
   their intent; master's changes stay.
2. Remove every conflict marker, then `git add` each resolved file.
3. For `test-graph.json`, run `uv run ci/test_test_graph_sorted.py --fix`. For
   Rust files, run `soldr cargo fmt --all`. `git add` what they change.
4. Stop. Do NOT run `git rebase --continue`, `--skip` or `--abort`, do not
   commit, do not push, do not touch files that are not conflicted (apart from
   step 3's formatting), and do not touch any GitHub PR.

If a conflict cannot be resolved without guessing at behaviour, leave that file
unstaged and say why in your final message. The caller then refuses the rebase.
