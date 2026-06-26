-- Runtime OOM hint: the deployed MCP server was killed by the kernel's OOM-killer
-- *while running* (not during build). The actionable fix is to raise the machine's
-- memory, which the existing build-time 'oom'/'killed' hints don't say (they suggest
-- shrinking dependencies / optimizing the build, which is wrong for a runtime crash).
--
-- The kernel OOM-killer always logs "Out of memory: Killed process <pid>", so the
-- ['out of memory', 'killed process'] pair uniquely identifies the runtime case.
-- Node's build-time OOM ("JavaScript heap out of memory") lacks "killed process",
-- so it won't false-match. Priority 90 > the generic 'oom' (80) / 'killed' (60) so
-- this more specific hint wins.

-- English
INSERT INTO error_hints (keywords, hint_message, priority, category, locale) VALUES
(ARRAY['out of memory', 'killed process'],
'💡 Hint: Your MCP server ran out of memory and was killed while running.

The deploy reached Fly.io, but the server process needed more RAM than the machine had, so it crash-looped instead of serving requests.

To fix it:
- Increase the memory in your server settings and redeploy.
- Browser-based servers (Playwright, Puppeteer, Selenium) need ~2GB.
- If you have hit your plan''s memory ceiling, upgrade the plan to allow a larger machine.', 90, 'resource', 'en');

-- Japanese
INSERT INTO error_hints (keywords, hint_message, priority, category, locale) VALUES
(ARRAY['out of memory', 'killed process'],
'💡 ヒント: MCPサーバーが稼働中にメモリ不足で強制終了されました。

デプロイ自体はFly.ioに到達しましたが、サーバープロセスがマシンのメモリを超過したため、リクエストを処理できずクラッシュを繰り返しています。

対処方法：
- サーバー設定でメモリを増やして再デプロイしてください。
- ブラウザ系サーバー（Playwright / Puppeteer / Selenium）は約2GB必要です。
- プランのメモリ上限に達している場合は、より大きなマシンを使えるプランにアップグレードしてください。', 90, 'resource', 'ja');
