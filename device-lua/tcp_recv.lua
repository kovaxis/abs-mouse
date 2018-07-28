local net,remote_id,client_fd,updates=...;
local socket=require "socket";
local util=require "util";
util.redirect_print(net.to_ui);

--Tcp socket rebuild hack
local client=socket.tcp();
client:connect("localhost",0);
client:setfd(client_fd);
client:setoption('tcp-nodelay',true);

local killed_locally=false;
local function get_updates()
  while true do
    local msg=updates:pop();
    if not msg then
      break;
    elseif msg.type=="kill_remote" then
      killed_locally=true;
    else
      error("invalid thread message to tcp receiver thread");
    end
  end
end

local err;
while true do
  local len;
  len,err=client:receive(4);
  if len then
    len=string.unpack(">I4",len);
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

get_updates();
if not killed_locally then
  print("failed to receive from "..remote_id..": "..err);
  net.to_server:push{type="kill_remote",remote_id=remote_id};
end
print("shutting down tcp receiver thread to "..remote_id);
