-- merm.nvim: Neovim integration plugin for merm diagram viewer
-- Implements bidirectional JSON-RPC 2.0 communication over Unix Domain Socket

local M = {}

M.config = {
    socket_path = nil,
    auto_sync_cursor = true,
    debounce_ms = 80,
}

local uv = vim.uv or vim.loop
local client = nil
local debounce_timer = nil

local function get_default_socket()
    local xdg_runtime = os.getenv("XDG_RUNTIME_DIR")
    if xdg_runtime then
        -- Look for any active merm socket in $XDG_RUNTIME_DIR/merm/
        local handle = uv.fs_scandir(xdg_runtime .. "/merm")
        if handle then
            while true do
                local name, type = uv.fs_scandir_next(handle)
                if not name then break end
                if name:match("^merm%-%d+%.sock$") then
                    return xdg_runtime .. "/merm/" .. name
                end
            end
        end
    end
    return nil
end

function M.connect(socket_path)
    local target_socket = socket_path or M.config.socket_path or get_default_socket()
    if not target_socket then
        vim.notify("[merm.nvim] No active merm socket found. Start merm first.", vim.log.levels.WARN)
        return false
    end

    client = uv.new_pipe(false)
    client:connect(target_socket, function(err)
        if err then
            vim.schedule(function()
                vim.notify("[merm.nvim] Connection failed: " .. err, vim.log.levels.ERROR)
            end)
            client = nil
        else
            vim.schedule(function()
                vim.notify("[merm.nvim] Connected to " .. target_socket, vim.log.levels.INFO)
            end)
        end
    end)
    return true
end

function M.send_notification(method, params)
    if not client then return end
    local payload = {
        jsonrpc = "2.0",
        method = method,
        params = params,
    }
    local raw = vim.json.encode(payload) .. "\n"
    client:write(raw)
end

function M.notify_cursor_moved()
    if not client then return end
    local current_file = vim.fn.expand("%:p")
    local cursor = vim.api.nvim_win_get_cursor(0)
    local cword = vim.fn.expand("<cword>")

    M.send_notification("merm/cursor_moved", {
        file = current_file,
        line = cursor[1],
        column = cursor[2],
        symbol = cword ~= "" and cword or nil,
    })
end

function M.setup(opts)
    M.config = vim.tbl_deep_extend("force", M.config, opts or {})

    vim.api.nvim_create_user_command("MermConnect", function(args)
        M.connect(args.args ~= "" and args.args or nil)
    end, { nargs = "?" })

    vim.api.nvim_create_user_command("MermReload", function()
        if not client then return end
        local lines = vim.api.nvim_buf_get_lines(0, 0, -1, false)
        local content = table.concat(lines, "\n")
        M.send_notification("merm/reload", {
            file = vim.fn.expand("%:p"),
            content = content,
        })
    end, {})

    if M.config.auto_sync_cursor then
        debounce_timer = uv.new_timer()
        local group = vim.api.nvim_create_augroup("MermSyncGroup", { clear = true })
        vim.api.nvim_create_autocmd({ "CursorMoved", "CursorMovedI" }, {
            group = group,
            pattern = { "*.md", "*.markdown", "*.mermaid", "*.mmd" },
            callback = function()
                if debounce_timer then
                    debounce_timer:start(M.config.debounce_ms, 0, vim.schedule_wrap(function()
                        M.notify_cursor_moved()
                    end))
                end
            end,
        })

        vim.api.nvim_create_autocmd("BufWritePost", {
            group = group,
            pattern = { "*.md", "*.markdown", "*.mermaid", "*.mmd" },
            callback = function()
                vim.cmd("MermReload")
            end,
        })
    end
end

return M
