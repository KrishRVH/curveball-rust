onClipEvent(enterFrame){
   myTarX = _root._xmouse;
   myTarY = _root._ymouse;
   myPos.x -= (myPos.x - myTarX) / 1.5;
   myPos.y -= (myPos.y - myTarY) / 1.5;
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
   _X = myPos.x;
   _Y = myPos.y;
   mySpeed.x = myPos.x - oldPos.x;
   mySpeed.y = myPos.y - oldPos.y;
   oldPos.x = myPos.x;
   oldPos.y = myPos.y;
   _parent.world.paddlePosX = myPos.x;
   _parent.world.paddlePosY = myPos.y;
   _parent.world.paddleSpeedX = mySpeed.x;
   _parent.world.paddleSpeedY = mySpeed.y;
}
