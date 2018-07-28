--TCP listen for connections
local net=...;
local socket=require "socket";
local util=require "util";
util.redirect_print(net.to_ui);

local listener,err=socket.bind("*",8517);
if listener then
  while true do
    local client,err=listener:accept();
    if client then
      client:setoption('tcp-nodelay',true);
      local ip,port=client:getpeername();
      local remote_id="tcp/"..ip.."/"..port;
      --Notify main server of new connection
      local sock_data={protocol="tcp",fd=client:getfd(),updates=love.thread.newChannel()};
      net.to_server:push{type="new_remote",remote_id=remote_id,sock_data=sock_data};
      --Spawn off a new thread dedicated to reading from this client
      local receiver=love.thread.newThread("tcp_recv.lua");
      receiver:start(net,remote_id,client:getfd(),sock_data.updates);
    else
      --Error accepting client!
      print("failed to accept tcp connection: "..err);
    end
  end
else
  print("failed to bind tcp server");
end
