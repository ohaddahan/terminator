-- Application-owned editor commands; the user's configuration is loaded normally.
vim.api.nvim_create_user_command('TerminatorCompareDisk', function()
  local path = vim.api.nvim_buf_get_name(0)
  if path == '' or vim.fn.isdirectory(path) == 1 then
    vim.notify('Select a file buffer to compare with disk', vim.log.levels.WARN)
    return
  end
  local ok, lines = pcall(vim.fn.readfile, path)
  if not ok then
    vim.notify('Cannot read the file on disk: ' .. path, vim.log.levels.ERROR)
    return
  end
  vim.cmd('diffthis')
  vim.cmd('vnew')
  vim.bo.buftype = 'nofile'
  vim.bo.bufhidden = 'wipe'
  vim.bo.swapfile = false
  vim.api.nvim_buf_set_lines(0, 0, -1, false, lines)
  vim.bo.modified = false
  vim.bo.modifiable = false
  vim.cmd('diffthis')
end, { force = true, desc = 'Compare unsaved buffer with a read-only disk snapshot' })
