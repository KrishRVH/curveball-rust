if(loading == "no")
{
   if(0 < winner)
   {
      winner = null;
      gotoAndPlay(104);
   }
   else if(winner == 0)
   {
      winner = null;
      gotoAndPlay(111);
   }
}
else
{
   gotoAndPlay(_currentframe - 1);
}
