# Troubleshooting Guide

## Common Issues

### Connection Errors

#### "Connection refused" error

**Causes:**
- Tauri app is not running
- Socket server failed to start
- Mismatched connection modes (IPC vs TCP)
- Port mismatch (TCP mode)

**Solutions:**
1. Verify the Tauri app is running in development mode
2. Check the app logs for socket server startup messages
3. Ensure both plugin and MCP server use the same connection type
4. For TCP: verify port numbers match on both sides

#### "Socket file not found" (IPC mode)

**Causes:**
- Socket file not created
- Wrong socket path configured
- Permission issues

**Solutions:**
1. Check `/tmp` directory for socket file (macOS/Linux)
2. Verify socket path in both plugin config and MCP server env
3. Check file permissions on socket path
4. Try TCP mode as alternative

### Permission Errors

#### "Permission denied" errors

**Platform-specific solutions:**

**Windows:**
- Ensure named pipe path follows Windows conventions
- Check Windows security settings

**Unix/Linux/macOS:**
- Check file permissions: `ls -la /tmp/tauri-mcp.sock`
- Ensure user has read/write access
- Consider using TCP mode to avoid file permissions

### Connection Stability

#### Connection drops after each request

**Causes:**
- Old plugin version without persistent connections
- Network interruption (TCP mode)
- Socket file corruption

**Solutions:**
1. Update to latest plugin version
2. Check Tauri app console for errors
3. For IPC: delete socket file and restart
4. For TCP: check firewall settings

## Testing Your Setup

### Using MCP Inspector

Test your configuration with the MCP Inspector tool:

**IPC Mode (default):**
```bash
cd mcp-server-ts
npx @modelcontextprotocol/inspector node build/index.js
```

**TCP Mode:**
```bash
cd mcp-server-ts
export TAURI_MCP_CONNECTION_TYPE=tcp
export TAURI_MCP_TCP_HOST=127.0.0.1
export TAURI_MCP_TCP_PORT=4000
npx @modelcontextprotocol/inspector node build/index.js
```

**Windows (TCP):**
```cmd
cd mcp-server-ts
set TAURI_MCP_CONNECTION_TYPE=tcp
set TAURI_MCP_TCP_HOST=127.0.0.1
set TAURI_MCP_TCP_PORT=4000
npx @modelcontextprotocol/inspector node build\index.js
```

### Verification Steps

1. **Check socket file exists** (IPC mode):
   ```bash
   ls -la /tmp/tauri-mcp.sock
   ```

2. **Check TCP port is listening** (TCP mode):
   ```bash
   netstat -an | grep 4000
   ```

3. **Test basic connectivity**:
   Use the `ping` tool in your MCP client to verify connection

4. **Check logs**:
   - Tauri app console output
   - MCP server stderr output
   - System logs for permission issues

## Getting Help

If you're still experiencing issues:

1. Check the [GitHub Issues](https://github.com/delorenj/tauri-plugin-mcp/issues)
2. Create a new issue with:
   - Operating system and version
   - Connection mode (IPC/TCP)
   - Complete error messages
   - Relevant log output
   - Steps to reproduce

## Environment Variables Reference

### MCP Server Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `TAURI_MCP_CONNECTION_TYPE` | `ipc` | Connection type: `ipc` or `tcp` |
| `TAURI_MCP_IPC_PATH` | `/tmp/tauri-mcp.sock` | IPC socket path (platform default if not set) |
| `TAURI_MCP_TCP_HOST` | `127.0.0.1` | TCP host address |
| `TAURI_MCP_TCP_PORT` | `9999` | TCP port number |
