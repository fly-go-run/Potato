use framework "Foundation"
use framework "ApplicationServices"

on run argv
	set xVal to (item 1 of argv) as real
	set yVal to (item 2 of argv) as real
	set eventMode to "click"
	if (count of argv) ≥ 3 then set eventMode to item 3 of argv
	set clickCount to 1
	if (count of argv) ≥ 4 then set clickCount to (item 4 of argv) as integer
	
	set pt to current application's CGPointMake(xVal, yVal)
	if eventMode is "move" then
		set moveEvt to current application's CGEventCreateMouseEvent(missing value, current application's kCGEventMouseMoved, pt, current application's kCGMouseButtonLeft)
		current application's CGEventPost(current application's kCGHIDEventTap, moveEvt)
		return "moved"
	end if
	
	repeat clickCount times
		set downEvt to current application's CGEventCreateMouseEvent(missing value, current application's kCGEventLeftMouseDown, pt, current application's kCGMouseButtonLeft)
		set upEvt to current application's CGEventCreateMouseEvent(missing value, current application's kCGEventLeftMouseUp, pt, current application's kCGMouseButtonLeft)
		current application's CGEventPost(current application's kCGHIDEventTap, downEvt)
		current application's CGEventPost(current application's kCGHIDEventTap, upEvt)
	end repeat
	return "clicked"
end run
