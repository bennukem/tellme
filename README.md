#### Tell me
Juste une API pour faire suivre un formulaire de contact à un destinataire. Principalement destiné aux sites statiques.

##### Pourquoi ?
Avec des solutions Comme Zola, HugoCMS, ou autre, vous pouvez héberger vos sites vitrines pour presque rien chez cloudflare pages par exemple. Le soucis d'un site statique, comme son nom l'indique, aucune intération.

##### Comment ?
L'idée et qu'une âme charitable heberge le service. Demandez à un chaton de framasoft, qui sait. Le service est fait pour être ouvert à tous.

##### Fonctionnement
L'utilisateur via une api, vas simplement entrer son email et recevoir en échange un token.

Ce Token sera à transmettre lors de l'envoi du fomulaire, cela permettra de faire la liaison avec votre email sans jamais la diffuser.

Pour l'instant, supprimer votre "compte" est tout aussi simple, un appel à l'API avec votre email et vous disparaissez du système. Pour l'instant, ce n'est pas vraiment l'idéal, on enverra un email pour une prochaine mise à jouer. L'email contiendra un token special pour la suppression.

##### Pour l'API, c'est en JSON que ça se passe
- POST /account
- DELETE /accound
- POST / Message

##### Privacy
Ca ne sauvegarde que :
- le token
- l'email
- le nombre de mail envoyés
- la dernière fois qu'il s'est passé quelque chose pour votre compte

Et oui c'est tout...