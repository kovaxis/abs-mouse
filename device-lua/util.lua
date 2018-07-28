require "love.thread";

local mutex_meta={__index={
  set=function(self,val)
    self:lock(function() return val end);
  end,
  get=function(self)
    return self[1]:peek();
  end,
  lock=function(self,callback)
    self[1]:performAtomic(function()
      self[1]:push(callback(self[1]:pop()));
    end)
  end,
}};
local function mutex(val)
  local ch=love.thread.newChannel();
  ch:push(val);
  return setmetatable({ch},mutex_meta);
end

return {
  mutex=mutex,
  redirect_print=function(channel)
    local real_print=print;
    function print(...)
      real_print(...);
      channel:push{type="log",...};
    end
  end,
};
