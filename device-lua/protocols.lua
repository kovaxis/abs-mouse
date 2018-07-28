local socket=require "socket";

--A table of protocol initialization functions.
--Each of this functions receives the `network` object and returns a protocol
--table, containing a single function: `new_socket`.
--
--The `new_socket` function returns another table containing a `send` function,
--taking a data string.
--On success the `send` function will return a truthy value.
--On failure it will return a falsey value followed by an error message.
--The `close` function will make its best effort to shut down the connection.
local protocols={};

function protocols.udp(net)
  --Create receiver thread
  local updates=love.thread.newChannel();
  local udp_recv=love.thread.newThread("udp_recv.lua");
  udp_recv:start(net,updates);
  
  --Create sending socket
  local udp=socket.udp();
  udp:setoption('reuseaddr',true);
  udp:setoption('reuseport',true);
  udp:setsockname("*",8517);
  
  --Create socket init function
  return {
    new_socket=function(sock_data)
      local remote_id=sock_data.remote_id;
      local addr,port=sock_data.addr,sock_data.port;
      return {
        send=function(data)
          return udp:sendto(data,addr,port);
        end,
        close=function()
          updates:push{type="kill_remote",remote_id=remote_id};
        end,
      };
    end,
  };
end

function protocols.tcp(net)
  --Create listener thread
  local tcp_listen=love.thread.newThread("tcp_listen.lua");
  tcp_listen:start(net);
  
  --Create socket init function
  return {
    new_socket=function(sock_data)
      local updates=sock_data.updates;
      --Socket rebuild hack
      local sock=socket.tcp();
      sock:connect("localhost",0);
      sock:setfd(sock_data.fd);
      sock:setoption('tcp-nodelay',true);
      return {
        send=function(data)
          return sock:send(love.data.pack(">s4",data));
        end,
        close=function()
          updates:push{type="kill_remote"};
          sock:close();
        end,
      };
    end,
  };
end

return protocols;
