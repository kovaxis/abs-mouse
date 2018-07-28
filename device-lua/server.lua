--Thin layer between UI and raw network
--Listens to incoming messages from the network and hands the 'sender' new incoming connections
local net=...;
local socket=require "socket";
local protocols=require "protocols";
local util=require "util";
util.redirect_print(net.to_ui);
require "love.timer";

local absm_version={major=1,minor=0};
local password="";
local width,height;

--Initialize protocols
do
  local protos={};
  for name,protocol in pairs(protocols) do
    protocols[name]=protocol(net);
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

--Send an update message to the UI thread.
local function ui_update_remote(remote,key,val)
  return net.to_ui:push{type="update_remote",remote_id=remote.remote_id,key=key,val=val};
end

--Parse headers from a header string
local function parse_headers(head_str,i)
  i=i or 1;
  local function next_header(head_str)
    if #head_str<i then
      return nil,nil;
    else
      local key,val,new_i=string.unpack(">s4s4",head_str,i);
      i=new_i;
      return key,val;
    end
  end
  return next_header,head_str;
end

--Place screen resolution info into the header table
local function place_screen_res(headers)
  headers.screen_res=string.pack(">ff",width,height);
end

--Send a server info update through the connection.
local function send_server_info(remote,info_headers)
  local msg={
    "sInf";
    string.pack(">I2I2",absm_version.major,absm_version.minor);
  };
  for key,val in pairs(info_headers) do
    msg[#msg+1]=string.pack(">s4s4",key,val);
  end
  remote:send(table.concat(msg));
end

--Call a function. If it errors, log it into the network log and kill the remote.
local function abortable(abortable_func,remote,...)
  local ok,err=pcall(abortable_func,remote,...);
  if ok then
    return err;
  else
    print("aborted "..remote.remote_id..": "..err);
    return kill_remote(remote);
  end
end

--Parse an opening message for remote.
--This is the first packet that should be send by the remote on connection.
local function parse_handshake_open(remote,data)
  if data:sub(1,4)~="absM" then
    error("invalid abs-m connection open message",0);
  end
  --Check version
  local major,minor=string.unpack(">I2I2",data,5);
  if major~=absm_version.major then
    error("incompatible abs-m protocol version:\n"..
      " remote ("..major.."."..minor..") != local ("..absm_version.major.."."..absm_version.minor..")",0);
  end
  --Now that version has been checked, there is not as much strain on compatibility
  local reportedPassword="";
  for key,val in parse_headers(data,9) do
    if key=="password" then
      reportedPassword=val;
    elseif key=="frame_delay" or key=="update_delay" then
      --Update UI fps or ups
      if #val>=4 then
        ui_update_remote(remote,key,string.unpack(">f",val));
      else
        print(key.." update header too short");
      end
    else
      print("unknown open header '"..key.."' = '"..val.."'");
    end
  end
  --Rudimentary security
  if password~=reportedPassword then
    error("password mismatch",0);
  end
  --Proceed with connection
  if remote.stage=="disconnected" then
    local headers={};
    place_screen_res(headers);
    send_server_info(remote,headers);
    remote.stage="connecting";
    remote.timeout_on=love.timer.getTime()+2;
    ui_update_remote(remote,"stage","connecting");
    print("setting up connection with "..remote.remote_id);
  end
end

--Parse a setup packet
local function parse_setup_info(remote,data)
  local function abort(why)
    print("connection to "..remote.remote_id.." aborted: "..why);
    return kill_remote(remote);
  end
  
  --Check packet type
  if data:sub(1,4)~="setp" then
    return abort("invalid abs-m connection setup message");
  end
  --Check header fields
  for key,val in parse_headers(data,5) do
    if key=="mapped_rect" then
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
  local pack_ty=data:sub(1,4);
  if remote.stage=="connected" then
  elseif remote.stage=="disconnected" then
    if pack_ty~="absM" then
      print("ignored message from "..remote.remote_id..", expecting a handshake-open");
      return;
    end
  elseif remote.stage=="connecting" then
    if pack_ty~="setp" then
      print("ignored message from "..remote.remote_id..", expecting a setup-info");
      return;
    end
  else
    error("invalid connection stage "..tostring(remote.stage));
  end
  --Parse a packet
  if pack_ty=="absM" then
    --Read some basic config and open remote if it's disconnected
    abortable(parse_handshake_open,remote,data);
  elseif pack_ty=="setp" then
    --Read setup info
    abortable(parse_setup_info,remote,data);
  elseif pack_ty=="ping" then
    --Reply a ping
    remote:send("repl"..data:sub(5));
  else
    print("unknown message type "..pack_ty.."' from "..remote.remote_id);
  end
end

--Regular ticks
local tick_delay=0.2;
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
    --Do a single network tick
    tick();
  elseif msg.type=="touch" then
    --Send touch data to connected remotes
    
  elseif msg.type=="key" then
    --Send keypress data to connected remotes
    
  elseif msg.type=="recv" then
    --Receive raw data through a remote socket
    local remote=assert(remotes[msg.remote_id],"received message before opening connection!");
    parse_message(remote,msg.data);
  elseif msg.type=="resize" then
    --Update screen dimensions and notify connected remotes of the change
    width,height=msg.width,msg.height;
    for remote_id,remote in pairs(remotes) do
      if remote.stage=="connected" then
        send_server_info(place_screen_res({}));
      end
    end
  elseif msg.type=="kill_remote" then
    --Forcibly remove remote
    local remote=remotes[msg.remote_id];
    if remote then
      kill_remote(remote);
    else
      print("redundant `kill_remote` for "..msg.remote_id.."!");
    end
  elseif msg.type=="new_remote" then
    --Add a new disconnected remote
    register_remote(msg.remote_id,msg.sock_data);
  else
    error("invalid thread message to server '"..msg.type.."'");
  end
end
