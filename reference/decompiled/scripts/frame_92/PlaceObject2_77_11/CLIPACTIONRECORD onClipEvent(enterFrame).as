onClipEvent(enterFrame){
   myPos.z = _parent.ballPosZ;
   if(myPos.z < 0)
   {
      myPos.z = 0;
   }
   VisPos.x = wx - (wx - myPos.x) * ((90 - Math.atan(myPos.z / varA) * 180 / 3.141592653589793) / 90);
   VisPos.y = wy - (wy - myPos.y) * ((90 - Math.atan(myPos.z / varA) * 180 / 3.141592653589793) / 90);
   _X = VisPos.x;
   _Y = VisPos.y;
   _alpha = (myPos.z - 100) / -1;
   _width = swidth * ((90 - Math.atan(myPos.z / varA) * 180 / 3.141592653589793) / 90);
   _height = sheight * ((90 - Math.atan(myPos.z / varA) * 180 / 3.141592653589793) / 90);
}
