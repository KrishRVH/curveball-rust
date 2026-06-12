on(release){
   if(Name != "enter here")
   {
      _parent.loading = "yes";
      alg = _parent.world.level + Name + _parent.world.score + "a83l9xj";
      loadVariables("enterscore.php?Score=" + _parent.world.score + "&Level=" + _parent.world.level + "&Name=" + Name + "&alg=" + alg,_parent);
   }
   tellTarget(_parent)
   {
      gotoAndStop("Submit");
      play();
   }
}
