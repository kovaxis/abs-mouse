--Thin layer between UI and raw network
--Listens to incoming messages from the network and hands the 'sender' new incoming connections
local net=...;
local socket=require "socket";
local protocols=require "protocols";
local util=require "util";
util.redirect_print(net.to_ui);
require "love.data";
require "love.timer";

local absm_version={major=1,minor=0};
local password="";

--Initialize protocols
do
  local protos={};
  for name,protocol in pairs(protocols) do
    protocols[name]=protocol();
    protos[#protos+1]=name;
  end
  print("initialized "..#protos.." protocols: "..table.concat(protos,", "));
end

--Keep track of remote connections
local remotes={};

--Remove a remote connection from the active connection list and notify anyone interested
local function kill_remote(remote)
  remote.sock.close();
  net.to_ui:push{type="kill_remote",remote_id=remote.remote_id};
  remotes[remote.remote_id]=nil;
end

--Send a message through a remote connection
local function send_on_remote(remote,data)
  local ok,err=remote.sock.send(data);
  if not ok then
    print("failed to send data to remote "..remote.remote_id);
    kill_remote(remote);
  end
end

--Create and register a remote connection from an ID and some rebuild data
local function register_remote(remote_id,sock_data)
  local remote={
    remote_id=remote_id,
    stage="disconnected",
    timeout_on=love.timer.getTime()+2,
  };
  --Create sender function on remote
  local protocol=assert(protocols[sock_data.protocol],"invalid protocol "..tostring(sock_data.protocol));
  local sock=protocol.new_socket(sock_data);
  remote.sock=sock;
  remote.send=send_on_remote;
  remotes[remote_id]=remote;
  --Notify UI of new connection
  net.to_ui:push{type="new_remote",remote_id=remote_id};
  print("new connection on "..remote_id..", waiting for setup message");
  return remote;
end

local function ui_update_remote(remote,key,val)
  return net.to_ui:push{type="update_remote",remote_id=remote.remote_id,key=key,val=val};
end

--Parse an opening message for remote.
--This is the first packet that should be send by the remote on connection.
local function open_remote(remote,data)
  local function abort(why)
    print("rejected "..remote.remote_id..": "..why);
    return kill_remote(remote);
  end
  
  if #data<8 then
    return abort("abs-m connection open message too short");
  end
  if data:sub(1,4)~="absM" then
    return abort("invalid abs-m connection open message");
  end
  --Check version
  local major,minor=love.data.unpack(">I2I2",data,5);
  if major~=absm_version.major then
    return abort("incompatible abs-m protocol version:\n"..
      " remote ("..major.."."..minor..") != local ("..absm_version.major.."."..absm_version.minor..")");
  end
  --Now that version has been checked, there is not as much strain on compatibility
  local reportedPassword="";
  for key,val in data:sub(9):gmatch("([^\1]*)\1([^\2]*)\2") do
    if key=="password" then
      reportedPassword=val;
    else
      print("unknown open header '"..key.."' = '"..val.."'");
    end
  end
  if password~=reportedPassword then
    return abort("password mismatch");
  end
  --Proceed with connection
  remote.stage="connecting";
  remote.timeout_on=love.timer.getTime()+2;
  ui_update_remote(remote,"stage","connecting");
  print("setting up connection with "..remote.remote_id);
end

--Parse a setup packet
local function setup_remote(remote,data)
  local function abort(why)
    print("connection to "..remote.remote_id.." aborted: "..why);
    return kill_remote(remote);
  end
  
  --Check packet type
  if data:sub(1,4)~="setp" then
    return abort("invalid abs-m connection setup message");
  end
  --Check header fields
  for key,val in data:sub(5):gmatch("([^\1]*)\1([^\2]*)\2") do
    if key=="mapped_area" then
      --Do some checking
      print("mapping area to "..val);
    else
      print("unknown setup header '"..key.."' = '"..val.."'");
    end
  end
  --Update connection status
  if remote.stage=="connecting" then
    remote.stage="connected";
    remote.timeout_on=false;
    ui_update_remote(remote,"stage","connected");
    print("handshake with "..remote.remote_id.." completed");
  end
end

--Parse a received network message
local function parse_message(remote,data)
  if remote.stage=="connected" then
    --Parse any packet
    local pack_ty=data:sub(1,4);
    if pack_ty=="setp" then
      --In-connection setup
      setup_remote(remote,data);
    elseif pack_ty=="ping" then
      --Reply a ping
      remote:send("repl"..data:sub(5));
    end
  elseif remote.stage=="disconnected" then
    --Expecting a connection-initiate message
    return open_remote(remote,data);
  elseif remote.stage=="connecting" then
    --Expecting a connection-setup message
    return setup_remote(remote,data);
  else
    error("invalid connection stage "..tostring(remote.stage));
  end
end

--Regular ticks
local tick_delay=0.1;
local function tick()
  --Check for connections that have timed out
  local now=love.timer.getTime();
  for remote_id,remote in pairs(remotes) do
    if remote.timeout_on and now>=remote.timeout_on then
      print("connection "..remote.remote_id.." timed out");
      kill_remote(remote);
    end
  end
end

while true do
  local msg=net.to_server:demand(tick_delay);
  if msg==nil then
    tick();
  elseif msg.type=="touch" then
    
  elseif msg.type=="key" then
    
  elseif msg.type=="recv" then
    local remote=assert(remotes[msg.remote_id],"received message before opening connection!");
    parse_message(remote,msg.data);
  elseif msg.type=="resize" then
    
  elseif msg.type=="kill_remote" then
    local remote=remotes[msg.remote_id];
    if remote then
      kill_remote(remote);
    end
  elseif msg.type=="new_remote" then
    register_remote(msg.remote_id,msg.sock_data);
  else
    error("invalid thread message to server '"..msg.type.."'");
  end
end
