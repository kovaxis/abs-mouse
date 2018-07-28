local ui=...;
require "love.data";
local socket=require "socket";

local version={major=1,minor=0};
local password="";
local msg_parse={
  --Initiate a connection
  [0x0001]=function(data)
    local i=2;
    local major,minor,i=love.data.unpack(">I2I2",data,i);
    if major==version.major then
      --Check connection headers
      local reportedPassword="";
      for key,val in data:sub(i):gmatch("[^\1]*\1[^\2]*\2") do
        if key=="password" then
          reportedPassword=val;
        end
      end
      --Rudimentary security
      if password==reportedPassword then
        --Connect remote
        net.notify_ui:push{type="connect_remote"};
        remote.connected=true;
        net.send:push()
      end
    else
      --Incompatible protocol versions
      kill_remote(remote);
    end
  end,
};

--Keep track of stuff and respond to requests
while true do
  local msg=net.recv:demand();
  --Identify sender
  local remote=remotes[msg.remote];
  if not remote then
    remote={remote=msg.remote,connected=false};
    remotes[msg.remote]=remote;
    net.notify_ui:push{type="new_remote",remote=msg.remote};
  end
  --Parse message
  local msg_ty=love.data.unpack(">I2",msg.data);
  msg_parse[msg_ty](remote,msg.data);
end