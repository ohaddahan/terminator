-- App-owned review profile. Never load user config, plugins, modelines or downloads.
local config = vim.json.decode(table.concat(vim.fn.readfile(vim.env.TERMINATOR_REVIEW_CONFIG), "\n"))
vim.opt.runtimepath = { config.runtime, vim.env.VIMRUNTIME }
vim.opt.packpath = {}
vim.opt.modeline = false
vim.opt.exrc = false
vim.opt.swapfile = false
vim.opt.backup = false
vim.opt.writebackup = false
vim.opt.undofile = false
vim.opt.shada = ""
vim.opt.termguicolors = true
vim.opt.mouse = "a"
vim.opt.number = true
vim.opt.showtabline = 0
vim.opt.laststatus = 2
vim.opt.background = "dark"
vim.cmd("syntax enable")
vim.api.nvim_set_hl(0, "Normal", { fg = "#D1D3D9", bg = "#191A1C" })
vim.api.nvim_set_hl(0, "DiffAdd", { bg = "#203B2A" })
vim.api.nvim_set_hl(0, "DiffDelete", { bg = "#452729" })
require("codediff").setup({
  diff = { max_computation_time_ms = 2000, highlight_added_deleted_files = true },
  keymaps = { view = {
    quit = false, diff_get = false, diff_put = false, toggle_stage = false,
    toggle_staged_view = false, stage_hunk = false, unstage_hunk = false,
    discard_hunk = false, open_in_prev_tab = false,
  } },
})
local function protect()
  for _, buf in ipairs(vim.api.nvim_list_bufs()) do
    local name = vim.api.nvim_buf_get_name(buf)
    if name == config.left or name == config.right then
      vim.bo[buf].readonly = true
      vim.bo[buf].modifiable = false
      vim.bo[buf].swapfile = false
      vim.keymap.set("n", "q", "<Cmd>qa<CR>", { buffer = buf, desc = "Close review" })
      for _, win in ipairs(vim.fn.win_findbuf(buf)) do
        local label = name == config.left and config.left_label or config.right_label
        vim.wo[win].statusline = " " .. label .. " · Read only %= %l:%c "
      end
    end
  end
end
vim.api.nvim_create_autocmd({ "BufReadPost", "BufEnter", "BufWinEnter" }, {
  callback = function() vim.schedule(protect) end,
})
vim.api.nvim_create_autocmd("VimEnter", { once = true, callback = function()
  local ok, err = pcall(function()
    -- Pinned internal API accepts literal filenames, including spaces and Ex metacharacters.
    require("codediff.commands.handlers.file_diff").run(config.left, config.right, {
      layout = "side-by-side", exit_on_close = true,
    })
    protect()
    vim.schedule(protect)
    vim.g.terminator_review_ready = true
  end)
  if not ok then
    vim.api.nvim_err_writeln("Could not open bundled CodeDiff: " .. tostring(err))
  end
end })
