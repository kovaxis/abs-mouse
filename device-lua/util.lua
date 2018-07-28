require "love.thread";
require "love.data";

local util={};

--Do some polyfill
table.unpack=unpack;
string.unpack=love.data.unpack;
function string.pack(fmt,...)
  return love.data.pack("string",fmt,...);
end

--Tool to redirect prints to a channel
function util.redirect_print(channel)
  local real_print=print;
  function print(...)
    real_print(...);
    channel:push{type="log",...};
  end
end

--Mutexes
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
function util.mutex(val)
  local ch=love.thread.newChannel();
  ch:push(val);
  return setmetatable({ch},mutex_meta);
end

return util;
