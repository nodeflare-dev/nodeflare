-- Add error hint for entry command required error (all runtimes with stdio transport)

-- English
INSERT INTO error_hints (keywords, hint_message, priority, category, locale) VALUES
(ARRAY['entry command', 'required', 'stdio'],
'💡 Hint: Startup command is required for stdio transport.

In the server settings, set the "Startup Command" field:
- Node.js: node index.js, npx @modelcontextprotocol/server-xxx
- Python: python main.py, uv run mcp-server
- Go: ./your-binary-name
- Rust: ./your-binary-name stdio

This command tells the system how to start your MCP server in stdio mode.', 100, 'entrypoint', 'en');

-- Japanese
INSERT INTO error_hints (keywords, hint_message, priority, category, locale) VALUES
(ARRAY['entry command', 'required', 'stdio'],
'💡 ヒント: stdioトランスポートを使用する場合、起動コマンドの設定が必要です。

サーバー設定の「起動コマンド」欄に入力してください：
- Node.js: node index.js, npx @modelcontextprotocol/server-xxx
- Python: python main.py, uv run mcp-server
- Go: ./バイナリ名
- Rust: ./バイナリ名 stdio

このコマンドで、MCPサーバーをstdioモードで起動する方法を指定します。', 100, 'entrypoint', 'ja');
