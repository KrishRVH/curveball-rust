onClipEvent(mouseDown){
   if(mySpeed.z == 0)
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
         mySpeed.z = speed;
         myCurve.x = (- pSpeedX) / curveAmount;
         myCurve.y = pSpeedY / curveAmount;
         if(Math.abs(myCurve.x) < 0.01)
         {
            if(pPosX < wx)
            {
               myCurve.x = 0.01;
            }
            else
            {
               myCurve.x = -0.01;
            }
         }
         if(Math.abs(myCurve.y) < 0.01)
         {
            if(wy < pPosY)
            {
               myCurve.y = 0.01;
            }
            else
            {
               myCurve.y = -0.01;
            }
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
   }
}
