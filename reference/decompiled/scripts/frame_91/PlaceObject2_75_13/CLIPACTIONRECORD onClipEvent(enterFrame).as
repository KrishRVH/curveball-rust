onClipEvent(enterFrame){
   bPosX = _parent.ballPosX;
   bPosY = _parent.ballPosY;
   bDirZ = _parent.ballDirZ;
   if(0 < bDirZ)
   {
      myTarX = bPosX;
      myTarY = bPosY;
      myPos.x -= (myPos.x - myTarX) / skillFactor;
      myPos.y -= (myPos.y - myTarY) / skillFactor;
   }
   else
   {
      myTarX = wx;
      myTarY = wy;
      myPos.x -= (myPos.x - myTarX) / 15;
      myPos.y -= (myPos.y - myTarY) / 15;
   }
   if(myPos.y - sheight / 2 < wtop)
   {
      myPos.y = wtop + sheight / 2;
   }
   else if(wbottom < myPos.y + sheight / 2)
   {
      myPos.y = wbottom - sheight / 2;
   }
   if(myPos.x - swidth / 2 < wleft)
   {
      myPos.x = wleft + swidth / 2;
   }
   else if(wright < myPos.x + swidth / 2)
   {
      myPos.x = wright - swidth / 2;
   }
   VisPos.x = wx - (wx - myPos.x) * ((90 - Math.atan(myPos.z / varA) * 180 / 3.141592653589793) / 90);
   VisPos.y = wy - (wy - myPos.y) * ((90 - Math.atan(myPos.z / varA) * 180 / 3.141592653589793) / 90);
   _X = VisPos.x;
   _Y = VisPos.y;
   _width = swidth * ((90 - Math.atan(myPos.z / varA) * 180 / 3.141592653589793) / 90);
   _height = sheight * ((90 - Math.atan(myPos.z / varA) * 180 / 3.141592653589793) / 90);
   mySpeed.x = myPos.x - oldPos.x;
   mySpeed.y = myPos.y - oldPos.y;
   oldPos.x = myPos.x;
   oldPos.y = myPos.y;
   _parent.world.enemyPosX = myPos.x;
   _parent.world.enemyPosY = myPos.y;
   _parent.world.enemySpeedX = mySpeed.x;
   _parent.world.enemySpeedY = mySpeed.y;
}
