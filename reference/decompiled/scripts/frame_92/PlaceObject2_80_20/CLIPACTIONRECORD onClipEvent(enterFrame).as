onClipEvent(enterFrame){
   if(ballStop != 1)
   {
      pSpeedX = _parent.world.paddleSpeedX;
      pSpeedY = _parent.world.paddleSpeedY;
      pPosX = _parent.world.paddlePosX;
      pPosY = _parent.world.paddlePosY;
      eSpeedX = _parent.world.enemySpeedX;
      eSpeedY = _parent.world.enemySpeedY;
      ePosX = _parent.world.enemyPosX;
      ePosY = _parent.world.enemyPosY;
      mySpeed.x += myCurve.x;
      mySpeed.y += myCurve.y;
      myPos.z += mySpeed.z;
      myPos.x += mySpeed.x;
      myPos.y -= mySpeed.y;
      if(myCurve.x != 0)
      {
         myCurve.x /= curveDecay;
      }
      if(myCurve.y != 0)
      {
         myCurve.y /= curveDecay;
      }
      radius = swidth / 2;
      if(myPos.y - radius < wtop)
      {
         myPos.y = wtop + radius;
         myCurve.y /= (curveDecay - 1) * 50 + 1;
         mySpeed.y = - mySpeed.y;
         _parent.wallBounce2.start(0,1);
      }
      else if(wbottom < myPos.y + radius)
      {
         myPos.y = wbottom - radius;
         myCurve.y /= (curveDecay - 1) * 50 + 1;
         mySpeed.y = - mySpeed.y;
         _parent.wallBounce2.start(0,1);
      }
      if(myPos.x - radius < wleft)
      {
         myPos.x = wleft + radius;
         myCurve.x /= (curveDecay - 1) * 50 + 1;
         mySpeed.x = - mySpeed.x;
         _parent.wallBounce1.start(0,1);
      }
      else if(wright < myPos.x + radius)
      {
         myPos.x = wright - radius;
         myCurve.x /= (curveDecay - 1) * 50 + 1;
         mySpeed.x = - mySpeed.x;
         _parent.wallBounce1.start(0,1);
      }
      if(wdepth < myPos.z)
      {
         if(hitTest(_parent.enemyPaddle))
         {
            if(ePosX + 7 < myPos.x)
            {
               if(myPos.y < ePosY)
               {
                  tellTarget(_parent.enemyPaddle)
                  {
                     gotoAndStop("UR");
                     play();
                  }
               }
               else if(myPos.y >= ePosY)
               {
                  tellTarget(_parent.enemyPaddle)
                  {
                     gotoAndStop("BR");
                     play();
                  }
               }
            }
            else if(myPos.x < ePosX - 7)
            {
               if(myPos.y < ePosY)
               {
                  tellTarget(_parent.enemyPaddle)
                  {
                     gotoAndStop("UL");
                     play();
                  }
               }
               else if(myPos.y >= ePosY)
               {
                  tellTarget(_parent.enemyPaddle)
                  {
                     gotoAndStop("BL");
                     play();
                  }
               }
            }
            else if(ePosX + 7 >= myPos.x)
            {
               if(ePosY + 5 >= myPos.y)
               {
                  if(myPos.y >= ePosY - 5)
                  {
                     tellTarget(_parent.enemyPaddle)
                     {
                        gotoAndStop("C");
                        play();
                     }
                  }
                  else if(myPos.x >= ePosX)
                  {
                     tellTarget(_parent.enemyPaddle)
                     {
                        gotoAndStop("UR");
                        play();
                     }
                  }
                  else
                  {
                     tellTarget(_parent.enemyPaddle)
                     {
                        gotoAndStop("UL");
                        play();
                     }
                  }
               }
               else if(myPos.x >= ePosX)
               {
                  tellTarget(_parent.enemyPaddle)
                  {
                     gotoAndStop("BR");
                     play();
                  }
               }
               else
               {
                  tellTarget(_parent.enemyPaddle)
                  {
                     gotoAndStop("BL");
                     play();
                  }
               }
            }
            myPos.z = wdepth;
            myCurve.x = eSpeedX / curveAmount;
            myCurve.y = (- eSpeedY) / curveAmount;
            mySpeed.z = - mySpeed.z;
            _parent.ePaddleBounce.start(0,1);
         }
         else
         {
            mySpeed.x = 0;
            mySpeed.y = 0;
            mySpeed.z = 0;
            myCurve.x = 0;
            myCurve.y = 0;
            play();
            _parent.world.enemyLives -= 1;
            tellTarget(_parent.enemyLives)
            {
               gotoAndPlay("L" + _parent.world.enemyLives);
            }
            _parent.missSound.start(0,1);
            ballStop = 1;
         }
      }
      else if(myPos.z < 0)
      {
         if(hitTest(_parent.userPaddle))
         {
            if(pPosX + 7 < myPos.x)
            {
               if(myPos.y < pPosY)
               {
                  tellTarget(_parent.userPaddle)
                  {
                     gotoAndStop("UR");
                     play();
                  }
               }
               else if(myPos.y >= pPosY)
               {
                  tellTarget(_parent.userPaddle)
                  {
                     gotoAndStop("BR");
                     play();
                  }
               }
            }
            else if(myPos.x < pPosX - 7)
            {
               if(myPos.y < pPosY)
               {
                  tellTarget(_parent.userPaddle)
                  {
                     gotoAndStop("UL");
                     play();
                  }
               }
               else if(myPos.y >= pPosY)
               {
                  tellTarget(_parent.userPaddle)
                  {
                     gotoAndStop("BL");
                     play();
                  }
               }
            }
            else if(pPosX + 7 >= myPos.x)
            {
               if(pPosY + 5 >= myPos.y)
               {
                  if(myPos.y >= pPosY - 5)
                  {
                     tellTarget(_parent.userPaddle)
                     {
                        gotoAndStop("C");
                        play();
                     }
                     _parent.world.score += _parent.world.accuracyBonus;
                     _parent.world.accuracyBonus -= _parent.world.accuracyDegrade;
                     if(_parent.world.accuracyBonus < 0)
                     {
                        _parent.world.accuracyBonus = 0;
                     }
                     tellTarget(_parent.bonus)
                     {
                        bonus = "Accuracy Bonus!";
                        gotoAndStop("bonus");
                        play();
                     }
                  }
                  else if(myPos.x >= pPosX)
                  {
                     tellTarget(_parent.userPaddle)
                     {
                        gotoAndStop("UR");
                        play();
                     }
                  }
                  else
                  {
                     tellTarget(_parent.userPaddle)
                     {
                        gotoAndStop("UL");
                        play();
                     }
                  }
               }
               else if(myPos.x >= pPosX)
               {
                  tellTarget(_parent.userPaddle)
                  {
                     gotoAndStop("BR");
                     play();
                  }
               }
               else
               {
                  tellTarget(_parent.userPaddle)
                  {
                     gotoAndStop("BL");
                     play();
                  }
               }
            }
            myPos.z = 0;
            myCurve.x = (- pSpeedX) / curveAmount;
            myCurve.y = pSpeedY / curveAmount;
            mySpeed.z = - mySpeed.z;
            _parent.world.score += _parent.world.hitScore;
            _parent.world.hitScore -= _parent.world.hitDegrade;
            if(_parent.world.hitScore < 0)
            {
               _parent.world.hitScore = 0;
            }
            if(0.1 < Math.abs(myCurve.x))
            {
               if(0.1 < Math.abs(myCurve.y))
               {
                  _parent.world.score += _parent.world.superCurveBonus;
                  _parent.world.superCurveBonus -= _parent.world.superCurveDegrade;
                  if(_parent.world.superCurveBonus < 0)
                  {
                     _parent.world.superCurveBonus = 0;
                  }
                  tellTarget(_parent.bonus)
                  {
                     bonus = "Super Curve Bonus!";
                     gotoAndStop("bonus");
                     play();
                  }
               }
               else
               {
                  _parent.world.score += _parent.world.curveBonus;
                  _parent.world.curveBonus -= _parent.world.curveDegrade;
                  if(_parent.world.curveBonus < 0)
                  {
                     _parent.world.curveBonus = 0;
                  }
                  tellTarget(_parent.bonus)
                  {
                     bonus = "Curve Bonus!";
                     gotoAndStop("bonus");
                     play();
                  }
               }
            }
            else if(0.05 < Math.abs(myCurve.y))
            {
               _parent.world.score += _parent.world.curveBonus;
               _parent.world.curveBonus -= _parent.world.curveDegrade;
               if(_parent.world.curveBonus < 0)
               {
                  _parent.world.curveBonus = 0;
               }
               tellTarget(_parent.bonus)
               {
                  bonus = "Curve Bonus!";
                  gotoAndStop("bonus");
                  play();
               }
            }
            else if(0.05 < Math.abs(myCurve.x))
            {
               _parent.world.score += _parent.world.curveBonus;
               _parent.world.curveBonus -= _parent.world.curveDegrade;
               if(_parent.world.curveBonus < 0)
               {
                  _parent.world.curveBonus = 0;
               }
               tellTarget(_parent.bonus)
               {
                  bonus = "Curve Bonus!";
                  gotoAndStop("bonus");
                  play();
               }
            }
            _parent.score = _parent.world.score;
            _parent.pPaddleBounce.start(0,1);
         }
         else
         {
            mySpeed.x = 0;
            mySpeed.y = 0;
            mySpeed.z = 0;
            myCurve.x = 0;
            myCurve.y = 0;
            play();
            _parent.world.playerLives -= 1;
            tellTarget(_parent.playerLives)
            {
               gotoAndPlay("L" + _parent.world.playerLives);
            }
            _parent.world.hitScore = 100;
            _parent.world.curveBonus = 50;
            _parent.world.superCurveBonus = 150;
            _parent.world.accuracyBonus = 100;
            ballStop = 1;
            _parent.missSound.start(0,1);
         }
      }
      VisPos.x = wx - (wx - myPos.x) * ((90 - Math.atan(myPos.z / varA) * 180 / 3.141592653589793) / 90);
      VisPos.y = wy - (wy - myPos.y) * ((90 - Math.atan(myPos.z / varA) * 180 / 3.141592653589793) / 90);
      _X = VisPos.x;
      _Y = VisPos.y;
      _height = _width = swidth * ((90 - Math.atan(myPos.z / varA) * 180 / 3.141592653589793) / 90);
      _parent.ballPosX = myPos.x;
      _parent.ballPosY = myPos.y;
      _parent.ballPosZ = myPos.z;
      _parent.ballDirX = mySpeed.x;
      _parent.ballDirY = mySpeed.y;
      _parent.ballDirZ = mySpeed.z;
      if(mySpeed.z != 0 && 0 < _parent.world.bonusDisplay)
      {
         _parent.world.bonus -= 1;
      }
      if(_parent.world.bonus < 0)
      {
         _parent.world.bonus = 10;
         _parent.world.bonusDisplay -= 25;
         _parent.bonusScore = _parent.world.bonusDisplay;
      }
   }
}
