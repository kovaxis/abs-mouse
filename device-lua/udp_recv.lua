--UDP receiving end
local net,updates=...;
local socket=require "socket";
local util=require "util";
util.redirect_print(net.to_ui);

local udp=socket.udp();
udp:setoption('reuseaddr',true);
udp:setoption('reuseport',true);
udp:setsockname("*",8517);

local remotes={};
local function update_remotes()
  while true do
    local msg=updates:pop();
    if not msg then
      break;
    elseif msg.type=="kill_remote" then
      print("udp thread notified of death of "..msg.remote_id);
      remotes[msg.remote_id]=nil;
    end
  end
end

while true do
  local ok,a,b=udp:receivefrom();
  if ok then
    --Received a message
    local msg,ip,port=ok,a,b;
    local remote_id="udp/"..ip.."/"..port;
    --Ensure our remote listing is up to date
    update_remotes();
    --Add connection if it doesn't exist yet
    if not remotes[remote_id] then
      local sock_data={protocol="udp",addr=ip,port=port};
      net.to_server:push{type="new_remote",remote_id=remote_id,sock_data=sock_data};
      remotes[remote_id]=true;
    end
    --Send packet data back into main server
    net.to_server:push{type="recv",remote_id=remote_id,data=msg};
  else
    --Socket error!
    local err=a;
    print("udp socket failed to receive: "..err);
  end
end
