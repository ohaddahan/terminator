-- Read the file owned by this tab, never whichever buffer happens to have focus.
local uv = vim.uv or vim.loop
local wanted = uv.fs_realpath(_A.path) or _A.path
for _, buf in ipairs(vim.api.nvim_list_bufs()) do
  if vim.api.nvim_buf_is_loaded(buf) then
    local name = vim.api.nvim_buf_get_name(buf)
    if name == _A.path or (uv.fs_realpath(name) or name) == wanted then
      local tick = vim.api.nvim_buf_get_changedtick(buf)
      local modified = vim.bo[buf].modified
      local previous = _A.previous
      if type(previous) == 'table' and previous.buffer == buf and previous.tick == tick
          and previous.modified == modified then
        return vim.fn.json_encode({ unchanged = true })
      end
      local count = vim.api.nvim_buf_line_count(buf)
      if vim.api.nvim_buf_get_offset(buf, count) > _A.limit then
        return vim.fn.json_encode({ error = 'Markdown preview is limited to 1 MiB' })
      end
      return vim.fn.json_encode({
        path = name, text = table.concat(vim.api.nvim_buf_get_lines(buf, 0, -1, false), '\n'),
        revision = { buffer = buf, tick = tick, modified = modified },
      })
    end
  end
end
return vim.fn.json_encode({ error = 'This file is no longer loaded in the editor' })
