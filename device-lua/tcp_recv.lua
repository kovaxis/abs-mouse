local net,remote_id,client_fd=...;
local socket=require "socket";
local util=require "util";
util.redirect_print(net.to_ui);
require "love.data";

--Tcp socket rebuild hack
local client=socket.tcp();
client:connect("localhost",0);
client:setfd(client_fd);
client:setoption('tcp-nodelay',true);

local err;
while true do
  local len;
  len,err=client:receive(4);
  if len then
    len=love.data.unpack(">I4",len);
    local data;
    data,err=client:receive(len);
    if data then
      net.to_server:push{type="recv",remote_id=remote_id,data=data};
    else
      break;
    end
  else
    break;
  end
end

print("failed to receive from "..remote_id..": "..err);
print("shutting down tcp connection");
net.to_server:push{type="kill_remote",remote_id=remote_id};
