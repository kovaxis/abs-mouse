local util=require "util";
local socket=require "socket";

--Whether to use the love2D v11 function based loop
--Seems to be much slower
local use_function_loop=false;

--Current settings
--Completely mutable, might change at any time
local frame_delay=1/10;
local update_delay=1/480;
local track_mouse="always"; --Either "always", "pressed" or "never"

local width,height;

--Logging
local new_log;
do
  local function write(log_lines,str)
    log_lines[#log_lines]=log_lines[#log_lines]..str:match("^[^\n]*");
    for line in str:gmatch("\n[^\n]*") do
      log_lines[#log_lines]=log_lines[#log_lines].."\n";
      table.remove(log_lines,1);
      log_lines[#log_lines+1]=line:sub(2);
    end
  end
  local function print(log_lines,...)
    local len=select('#',...);
    for i=1,len do
      local arg=select(i,...);
      log_lines:write(tostring(arg));
      if i<len then
        log_lines:write "  ";
      else
        log_lines:write "\n";
      end
    end
  end
  local function take_lines(log_lines,width,count)
    --Get bottom line
    local limit=#log_lines;
    if log_lines[limit]=="" then limit=limit-1; end
    --Get top line
    local take_from=limit;
    while count>=1 and take_from>0 do
      --Check how many lines does this single log line take
      local cost;
      do
        local _,wrapped_lines=love.graphics.getFont():getWrap(log_lines[take_from],width);
        cost=#wrapped_lines;
      end
      --Take line if possible
      if count>=cost then
        --Take it
        count=count-cost;
        take_from=take_from-1;
      else
        --Too expensive
        break;
      end
    end
    --Return concatenation of lines
    return table.concat(log_lines,"",take_from+1,limit);
  end
  function new_log()
    local line_cap=30;
    
    local log_lines={write=write,take_lines=take_lines,print=print};
    for i=1,line_cap do
      if i==line_cap then
        log_lines[i]="";
      else
        log_lines[i]="\n";
      end
    end
    return log_lines;
  end
end
local local_log=new_log();
local net_log=new_log();
--Patch print
local real_print=print;
function print(...)
  real_print(...);
  return local_log:print(...)
end

--Networking
local network={
  --Updates from server to UI
  to_ui=love.thread.newChannel();
  --Updates to server (events from the UI side and messages from the lower level network interfaces)
  to_server=love.thread.newChannel();
};
local remotes={};
local server=love.thread.newThread("server.lua");
server:start(network);
local function network_tick()
  while true do
    local msg=network.to_ui:pop();
    if msg then
      if msg.type=="new_remote" then
        remotes[msg.remote_id]={stage="disconnected"};
      elseif msg.type=="kill_remote" then
        remotes[msg.remote_id]=nil;
      elseif msg.type=="update_remote" then
        if msg.key=="stage" then
          remotes[msg.remote_id].stage=msg.val;
        elseif msg.key=="frame_delay" then
          frame_delay=msg.val;
        elseif msg.key=="update_delay" then
          update_delay=msg.val;
        else
          error("invalid cross-thread remote update '"..tostring(msg.key).."' = '"..tostring(msg.val).."'",0);
        end
      elseif msg.type=="log" then
        net_log:print(unpack(msg));
      else
        error("invalid thread message '"..msg.type.."' to ui");
      end
    else
      break;
    end
  end
end
function love.threaderror(thread,err)
  --Propagate error to main thread: sub-threads shouldn't error
  error(err,0);
end

--Input management
local fastest_touch=nil;
local report_touch,report_key,set_size;
do
  local last_touch=nil;
  function report_touch(now,x,y)
    network.to_server:push{type="touch",x=x,y=y};
    print("touch on ["..x..", "..y.."]");
    if last_touch then
      local delay=now-last_touch;
      if delay>0 and (not fastest_touch or fastest_touch>delay) then
        fastest_touch=delay;
      end
    end
    last_touch=now;
  end
  function report_key(now,key,scancode,is_down)
    network.to_server:push{type="key",key=key,scancode=scancode,is_down=is_down};
    print("key ["..key..", "..scancode.."]");
  end
  function set_size(w,h)
    width,height=w,h;
    network.to_server:push{type="resize",width=w,height=h};
    print("set size to ["..w..", "..h.."]");
  end
end

--Input abstraction
local last_updates={};
local update;
do
  local mouse_is_down=false;
  local handle={};
  
  function handle.mousepressed(now,x,y)
    mouse_is_down=true;
    if track_mouse~="never" then
      report_touch(now,x,y);
    end
  end
  function handle.mousereleased(now)
    mouse_is_down=false;
  end
  function handle.mousemoved(now,x,y)
    if track_mouse=="always" or (track_mouse=="pressed" and mouse_is_down) then
      report_touch(now,x,y);
    end
  end
  
  function handle.keypressed(now,key,scancode,is_repeat)
    if is_repeat then return end
    report_key(now,key,scancode,true);
  end
  function handle.keyreleased(now,key,scancode)
    report_key(now,key,scancode,false);
  end
  
  function handle.resize(now,w,h)
    set_size(w,h);
  end
  
  function update()
    local update_time=love.timer.getTime();
    last_updates[#last_updates+1]=update_time;
    love.event.pump();
    for ev, a,b,c,d,e,f in love.event.poll() do
      if ev=="quit" then
        return a or 0;
      else
        local handler=handle[ev];
        if handler then
          handler(update_time,a,b,c,d,e,f);
        else
          love.handlers[ev](a,b,c,d,e,f);
        end
      end
    end
  end
end

--Rendering
local last_intervals={};
local avg_intervals={};
local render;
do
  local last_renders={};
  function render()
    local render_start=love.timer.getTime();
    --Remove renders that are too far back
    while #last_renders>0 do
      if last_renders[1]<render_start-1 then
        --Too far back, no longer relevant
        table.remove(last_renders,1);
      else
        break;
      end
    end
    while #last_updates>0 do
      if last_updates[1]<render_start-1 then
        table.remove(last_updates,1);
      else
        break;
      end
    end
    --Add this render
    last_renders[#last_renders+1]=render_start;
    --Get amount of renders in the last second
    local real_fps=#last_renders;
    local real_ups=#last_updates;
    
    --Parse messages from the networking thread cluster
    network_tick();
    
    --Get average interval usage
    if #last_intervals>0 then
      --Set all values to 0
      local interval_count=#last_intervals[1];
      for j=1,interval_count do
        avg_intervals[j]=0;
      end
      avg_intervals.total=0;
      --Add up all values
      for i=1,#last_intervals do
        local intervals=last_intervals[i];
        for j=1,interval_count do
          avg_intervals[j]=avg_intervals[j]+intervals[j];
        end
        avg_intervals.total=avg_intervals.total+intervals.total;
      end
      --Calculate average and setup for rendering
      local pie_width=width/2;
      for j=1,interval_count do
        local interval_width=avg_intervals[j]/#last_intervals*pie_width;
        avg_intervals[j]=interval_width;
      end
      avg_intervals.total=avg_intervals.total/#last_intervals;
    end
    
    --Preparation
    love.graphics.origin();
    love.graphics.clear(0.2,0.2,0.2,1);
    local font_height=love.graphics.getFont():getHeight();
    
    --Print logs
    local function print_log(log,x,bottom_y,w,h)
      local line_count=math.floor(h/font_height);
      h=line_count*font_height;
      love.graphics.printf(log:take_lines(w,line_count),x,bottom_y-h,w);
    end
    love.graphics.setColor(1,1,0);
    print_log(local_log,width/2,height/2,width/2,height/2);
    love.graphics.setColor(0,1,0);
    print_log(net_log,width/2,height,width/2,height/2);
    
    --Print stats
    do
      love.graphics.setColor(1,1,1);
      local y=0;
      local function stat(str)
        love.graphics.print(str,0,y);
        y=y+font_height;
      end
      stat("FPS: "..real_fps.."/"..(1/frame_delay));
      stat("UPS: "..real_ups.."/"..(1/update_delay));
      local fastest;
      if fastest_touch then
        fastest=math.floor(1/fastest_touch).."Hz";
      else
        fastest="no info yet";
      end
      stat("Peak touch event frequency: "..fastest);
      do
        stat("Time usage (inter-loop, update, render, sleep)");
        local x=0;
        local colors={{0.5,0.5,0.5},{0,1,1},{1,0,0},{0,1,0}};
        for i=1,#avg_intervals do
          local w=avg_intervals[i];
          love.graphics.setColor(colors[i]);
          love.graphics.rectangle("fill",x,y,w,font_height);
          x=x+w;
        end
        love.graphics.setColor(1,1,1);
        y=y+font_height;
      end
      if next(remotes) then
        stat("Connections:");
        for remote_id,remote in pairs(remotes) do
          stat(" "..remote_id);
        end
      else
        stat("No connections yet");
      end
    end
    
    --Final presentation
    love.graphics.present();
  end
end

--Main loop
function love.run()
  print("abs-mouse starting up")
  if not width or not height then
    set_size(love.graphics.getWidth(),love.graphics.getHeight());
  end
  print(" loaded");
  local loop_start_time=love.timer.getTime();
  local next_update=loop_start_time;
  local next_frame=loop_start_time;
  local awake_since=loop_start_time;
  local prev_post_sleep=loop_start_time;
  
  return function()
    local now;
    local timepoints={prev_post_sleep};
    local function timepoint()
      now=love.timer.getTime();
      timepoints[#timepoints+1]=now;
    end
    --Update
    timepoint();
    if now>=next_update then
      local quit=update();
      if quit then return quit; end
      next_update=next_update+update_delay;
    end
    --Render
    timepoint();
    if now>=next_frame then
      render();
      next_frame=next_frame+frame_delay;
    end
    --Sleep
    timepoint();
    local sleep_for=math.min(next_update,next_frame)-now;
    local update_awake_since;
    if sleep_for>0 then
      --In time, sleep for the required period
      love.timer.sleep(sleep_for);
      update_awake_since=true;
    elseif now-awake_since>0.5 then
      --Very late, bring `next_update` and `next_frame` up to date
      next_update=math.max(next_update,now);
      next_frame=math.max(next_frame,now);
      update_awake_since=true;
      print("can't keep up! skipping some updates");
    else
      --A bit late, but still in time
      --We will probably pick up the pace in the next iterations
      update_awake_since=false;
    end
    
    timepoint();
    if update_awake_since then
      awake_since=now;
    end
    prev_post_sleep=now;
    --Create tick stats
    do
      local max_interval_records=256;
      --Get a table for the intervals, recycling from the previous intervals if available
      local intervals;
      if #last_intervals<max_interval_records then
        intervals={};
      else
        intervals=table.remove(last_intervals,1);
      end
      
      --Normalize intervals so they add up to 1
      for i=1,#timepoints-1 do
        intervals[i]=timepoints[i+1]-timepoints[i];
      end
      intervals.total=0;
      for i=1,#intervals do
        intervals.total=intervals.total+intervals[i];
      end
      for i=1,#intervals do
        intervals[i]=intervals[i]/intervals.total;
      end
      
      --Add this interval record to the list
      last_intervals[#last_intervals+1]=intervals;
    end
  end
end
